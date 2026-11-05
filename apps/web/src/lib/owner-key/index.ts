const order = 0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551n;
const defaultIterations = 600_000;
const maxIterations = 10_000_000;
const format = 'at.motet.owner-key.backup';

const instructions =
	'Keep this file and its passphrase in separate safe places. To recover, import this file and enter the passphrase. Losing both copies of the key freezes the World identity.';

export class BackupError extends Error {
	constructor(
		public readonly code:
			'invalid-file' | 'world-mismatch' | 'key-mismatch' | 'authentication-failed'
	) {
		super(code === 'authentication-failed' ? 'Wrong passphrase or damaged backup' : code);
		this.name = 'BackupError';
	}
}

function encode(bytes: Uint8Array): string {
	return btoa(String.fromCharCode(...bytes))
		.replaceAll('+', '-')
		.replaceAll('/', '_')
		.replace(/=+$/, '');
}

function decode(value: string): Uint8Array<ArrayBuffer> {
	if (!/^[A-Za-z0-9_-]*$/.test(value) || value.length % 4 === 1)
		throw new BackupError('invalid-file');

	try {
		const binary = atob(value.replaceAll('-', '+').replaceAll('_', '/'));

		return Uint8Array.from(binary, (character) => character.charCodeAt(0));
	} catch {
		throw new BackupError('invalid-file');
	}
}

function base58(bytes: Uint8Array): string {
	const alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
	let number = 0n;

	for (const byte of bytes) number = number * 256n + BigInt(byte);
	let result = '';

	while (number > 0n) {
		result = alphabet[Number(number % 58n)] + result;
		number /= 58n;
	}

	for (const byte of bytes) {
		if (byte !== 0) break;
		result = `1${result}`;
	}

	return result;
}

/** Generate the owner's device-held key. Extraction is required for encrypted backup. */
export async function generate(): Promise<CryptoKeyPair> {
	return crypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, [
		'sign',
		'verify'
	]);
}

/** Encode a P-256 public key as the compressed multicodec did:key. */
export async function didKey(publicKey: CryptoKey): Promise<string> {
	const point = new Uint8Array(await crypto.subtle.exportKey('raw', publicKey));

	if (point.length !== 65 || point[0] !== 4) throw new Error('Expected a P-256 public key');
	const encoded = new Uint8Array(35);
	encoded.set([0x80, 0x24, 2 | (point[64] & 1)]);
	encoded.set(point.subarray(1, 33), 3);

	return `did:key:z${base58(encoded)}`;
}

/** Sign the decoded DAG-CBOR signing bytes; return PLC's unpadded base64url compact signature. */
export async function sign(privateKey: CryptoKey, signingBytes: BufferSource): Promise<string> {
	const signature = new Uint8Array(
		await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, privateKey, signingBytes)
	);

	if (signature.length !== 64) throw new Error('Expected a compact P-256 signature');
	let s = 0n;

	for (const byte of signature.subarray(32)) s = (s << 8n) | BigInt(byte);

	if (s > order / 2n) {
		s = order - s;

		for (let index = 63; index >= 32; index--) {
			signature[index] = Number(s & 255n);
			s >>= 8n;
		}
	}

	return encode(signature);
}

async function encryptionKey(
	passphrase: string,
	salt: Uint8Array<ArrayBuffer>,
	iterations: number
): Promise<CryptoKey> {
	const material = await crypto.subtle.importKey(
		'raw',
		new TextEncoder().encode(passphrase),
		'PBKDF2',
		false,
		['deriveKey']
	);

	return crypto.subtle.deriveKey(
		{ name: 'PBKDF2', hash: 'SHA-256', salt, iterations },
		material,
		{ name: 'AES-GCM', length: 256 },
		false,
		['encrypt', 'decrypt']
	);
}

function associatedData(worldDid: string, keyDid: string): Uint8Array<ArrayBuffer> {
	return new TextEncoder().encode(JSON.stringify([format, 1, worldDid, keyDid]));
}

/** Return a versioned encrypted JSON backup. The passphrase is never stored. */
export async function exportBackup({
	key,
	worldDid,
	passphrase,
	iterations = defaultIterations
}: {
	key: CryptoKeyPair;
	worldDid: string;
	passphrase: string;
	iterations?: number;
}): Promise<string> {
	if (!Number.isSafeInteger(iterations) || iterations < 1 || iterations > maxIterations)
		throw new Error('Invalid PBKDF2 iteration count');

	const keyDid = await didKey(key.publicKey);
	const salt = crypto.getRandomValues(new Uint8Array(16));
	const iv = crypto.getRandomValues(new Uint8Array(12));
	const jwk = await crypto.subtle.exportKey('jwk', key.privateKey);

	const ciphertext = await crypto.subtle.encrypt(
		{ name: 'AES-GCM', iv, additionalData: associatedData(worldDid, keyDid) },
		await encryptionKey(passphrase, salt, iterations),
		new TextEncoder().encode(JSON.stringify(jwk))
	);

	return JSON.stringify(
		{
			format,
			version: 1,
			worldDid,
			didKey: keyDid,
			kdf: { name: 'PBKDF2-SHA-256', iterations, salt: encode(salt) },
			cipher: {
				name: 'AES-256-GCM',
				iv: encode(iv),
				ciphertext: encode(new Uint8Array(ciphertext))
			},
			instructions
		},
		null,
		2
	);
}

// This is the untrusted JSON boundary; field contracts are checked below.
/* oxlint-disable anti-slop/no-unsafe-dictionary-type, anti-slop/no-runtime-typeof */
function record(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}
/* oxlint-enable anti-slop/no-unsafe-dictionary-type, anti-slop/no-runtime-typeof */

/** Decrypt and validate a backup against the expected World and rotation key.
 * Authentication failure cannot distinguish a wrong passphrase from file tampering.
 */
export async function verifyBackup(
	file: string,
	passphrase: string,
	expected: { worldDid: string; didKey: string }
): Promise<CryptoKeyPair> {
	let parsed: unknown;

	try {
		parsed = JSON.parse(file);
	} catch {
		throw new BackupError('invalid-file');
	}

	// Validate the file before any cryptographic operation.
	/* oxlint-disable anti-slop/no-runtime-typeof */
	if (
		!record(parsed) ||
		parsed.format !== format ||
		parsed.version !== 1 ||
		typeof parsed.worldDid !== 'string' ||
		typeof parsed.didKey !== 'string' ||
		!record(parsed.kdf) ||
		parsed.kdf.name !== 'PBKDF2-SHA-256' ||
		typeof parsed.kdf.iterations !== 'number' ||
		!Number.isSafeInteger(parsed.kdf.iterations) ||
		parsed.kdf.iterations < 1 ||
		parsed.kdf.iterations > maxIterations ||
		typeof parsed.kdf.salt !== 'string' ||
		!record(parsed.cipher) ||
		parsed.cipher.name !== 'AES-256-GCM' ||
		typeof parsed.cipher.iv !== 'string' ||
		typeof parsed.cipher.ciphertext !== 'string'
	)
		throw new BackupError('invalid-file');
	/* oxlint-enable anti-slop/no-runtime-typeof */

	if (parsed.worldDid !== expected.worldDid) throw new BackupError('world-mismatch');

	if (parsed.didKey !== expected.didKey) throw new BackupError('key-mismatch');
	const salt = decode(parsed.kdf.salt);
	const iv = decode(parsed.cipher.iv);
	const ciphertext = decode(parsed.cipher.ciphertext);

	if (salt.length !== 16 || iv.length !== 12 || ciphertext.length < 16)
		throw new BackupError('invalid-file');
	let plaintext: ArrayBuffer;

	try {
		plaintext = await crypto.subtle.decrypt(
			{ name: 'AES-GCM', iv, additionalData: associatedData(parsed.worldDid, parsed.didKey) },
			await encryptionKey(passphrase, salt, parsed.kdf.iterations),
			ciphertext
		);
	} catch {
		throw new BackupError('authentication-failed');
	}

	try {
		const jwk: unknown = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(plaintext));

		// The decrypted JSON is still untrusted until its key shape is checked.
		/* oxlint-disable anti-slop/no-runtime-typeof */
		if (
			!record(jwk) ||
			jwk.kty !== 'EC' ||
			jwk.crv !== 'P-256' ||
			typeof jwk.x !== 'string' ||
			typeof jwk.y !== 'string' ||
			typeof jwk.d !== 'string'
		)
			throw new BackupError('invalid-file');
		/* oxlint-enable anti-slop/no-runtime-typeof */

		const privateKey = await crypto.subtle.importKey(
			'jwk',
			{ kty: 'EC', crv: 'P-256', x: jwk.x, y: jwk.y, d: jwk.d },
			{ name: 'ECDSA', namedCurve: 'P-256' },
			true,
			['sign']
		);

		const publicKey = await crypto.subtle.importKey(
			'jwk',
			{ kty: 'EC', crv: 'P-256', x: jwk.x, y: jwk.y },
			{ name: 'ECDSA', namedCurve: 'P-256' },
			true,
			['verify']
		);

		if ((await didKey(publicKey)) !== expected.didKey) throw new BackupError('key-mismatch');
		const challenge = crypto.getRandomValues(new Uint8Array(32));
		const signature = decode(await sign(privateKey, challenge));

		if (
			!(await crypto.subtle.verify(
				{ name: 'ECDSA', hash: 'SHA-256' },
				publicKey,
				signature,
				challenge
			))
		)
			throw new BackupError('key-mismatch');

		return { privateKey, publicKey };
	} catch (error) {
		if (error instanceof BackupError) throw error;
		throw new BackupError('invalid-file');
	}
}
