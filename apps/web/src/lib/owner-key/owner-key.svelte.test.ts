import { describe, expect, it } from 'vitest';
import { p256 } from '@noble/curves/p256';
import { BackupError, didKey, exportBackup, generate, sign, verifyBackup } from './index';

const worldDid = 'did:plc:abcdefghijklmnopqrstuvwx';
const passphrase = 'an owner recovery passphrase';

function decode(value: string): Uint8Array {
	return Uint8Array.from(atob(value.replaceAll('-', '+').replaceAll('_', '/')), (character) =>
		character.charCodeAt(0)
	);
}

function encode(bytes: Uint8Array): string {
	return btoa(String.fromCharCode(...bytes))
		.replaceAll('+', '-')
		.replaceAll('/', '_')
		.replace(/=+$/, '');
}

function publicPoint(did: string): Uint8Array {
	const alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
	let value = 0n;

	for (const char of did.slice('did:key:z'.length))
		value = value * 58n + BigInt(alphabet.indexOf(char));
	const bytes = new Uint8Array(35);

	for (let index = bytes.length - 1; index >= 0; index--) {
		bytes[index] = Number(value & 255n);
		value >>= 8n;
	}

	expect(bytes.slice(0, 2)).toEqual(new Uint8Array([0x80, 0x24]));

	return bytes.slice(2);
}

describe('World owner rotation key', () => {
	it('signatures verify independently and remain low-S, even when WebCrypto produces high-S', async () => {
		const key = await generate();
		const point = publicPoint(await didKey(key.publicKey));

		for (let i = 0; i < 128; i++) {
			const message = crypto.getRandomValues(new Uint8Array(i));
			const signature = decode(await sign(key.privateKey, message));
			const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', message));
			expect(signature.length).toBe(64);
			expect(p256.verify(signature, digest, point, { lowS: true })).toBe(true);
		}
	});

	it('a backup restores the same key and signs independently, rather than restoring another key', async () => {
		const key = await generate();
		const expected = { worldDid, didKey: await didKey(key.publicKey) };
		const file = await exportBackup({ key, worldDid, passphrase });
		// A weaker default would make an offline passphrase attack cheaper.
		expect(JSON.parse(file).kdf.iterations).toBeGreaterThanOrEqual(600_000);
		const restored = await verifyBackup(file, passphrase, expected);
		const message = new TextEncoder().encode('recovered owner key');
		const signature = decode(await sign(restored.privateKey, message));
		const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', message));
		expect(p256.verify(signature, digest, publicPoint(expected.didKey), { lowS: true })).toBe(true);
		expect(await didKey(restored.publicKey)).toBe(expected.didKey);
	});

	it('rejects every altered ciphertext and salt byte, preventing silent corrupted recovery', async () => {
		const key = await generate();
		const expected = { worldDid, didKey: await didKey(key.publicKey) };

		const original = JSON.parse(
			await exportBackup({ key, worldDid, passphrase, iterations: 1000 })
		);

		for (const [section, field] of [
			['kdf', 'salt'],
			['cipher', 'ciphertext'],
			['cipher', 'iv']
		]) {
			const bytes = decode(original[section][field]);

			for (let index = 0; index < bytes.length; index++) {
				const changed = structuredClone(original);
				const altered = bytes.slice();
				altered[index] ^= 1;
				changed[section][field] = encode(altered);
				await expect(
					verifyBackup(JSON.stringify(changed), passphrase, expected)
				).rejects.toMatchObject({ code: 'authentication-failed' });
			}
		}
	}, 120_000);

	it('authenticates every byte of both World and key identities even when the expected identity changes too', async () => {
		const key = await generate();
		const expected = { worldDid, didKey: await didKey(key.publicKey) };

		const original = JSON.parse(
			await exportBackup({ key, worldDid, passphrase, iterations: 1000 })
		);

		for (const field of ['worldDid', 'didKey'] as const) {
			for (let index = 0; index < original[field].length; index++) {
				const value = original[field];

				const altered =
					value.slice(0, index) + (value[index] === 'a' ? 'b' : 'a') + value.slice(index + 1);

				const changed = { ...original, [field]: altered };
				await expect(
					verifyBackup(JSON.stringify(changed), passphrase, { ...expected, [field]: altered })
				).rejects.toMatchObject({ code: 'authentication-failed' });
			}
		}
	}, 120_000);

	it('refuses a wrong World, a changed key identity, or an incorrect passphrase', async () => {
		const key = await generate();
		const expected = { worldDid, didKey: await didKey(key.publicKey) };
		const file = await exportBackup({ key, worldDid, passphrase, iterations: 1000 });
		await expect(verifyBackup(file, 'wrong passphrase', expected)).rejects.toMatchObject({
			code: 'authentication-failed'
		});
		await expect(
			verifyBackup(file, passphrase, { ...expected, worldDid: 'did:plc:anotherworld' })
		).rejects.toMatchObject({ code: 'world-mismatch' });
		const changed = JSON.parse(file);
		changed.didKey += '1';
		await expect(verifyBackup(JSON.stringify(changed), passphrase, expected)).rejects.toMatchObject(
			{ code: 'key-mismatch' }
		);
		await expect(
			verifyBackup(JSON.stringify(changed), passphrase, { ...expected, didKey: changed.didKey })
		).rejects.toBeInstanceOf(BackupError);
	});

	it('emits a browser signature for the fixed cross-language P-256 test key', async () => {
		const jwk = {
			kty: 'EC',
			crv: 'P-256',
			x: 'axfR8uEsQkf4vOblY6RA8ncDfYEt6zOg9KE5RdiYwpY',
			y: 'T-NC4v4af5uO5-tKfA-eFivOM1drMV7Oy7ZAaDe_UfU',
			d: 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAE'
		};

		const privateKey = await crypto.subtle.importKey(
			'jwk',
			jwk,
			{ name: 'ECDSA', namedCurve: 'P-256' },
			false,
			['sign']
		);

		const message = new TextEncoder().encode('motet owner signing interoperability');
		const signature = await sign(privateKey, message);
		const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', message));

		const fixed =
			'uHdj6i0snbfhBFwoqfMs_40G5YzlZJjii5lXCFimfQcwhHJ3KgUs81p6SENc77zTrK_qNhAUHqD8a1Ub5tZLNA';

		const point = p256.getPublicKey(new Uint8Array([...new Uint8Array(31), 1]));
		expect(p256.verify(decode(fixed), digest, point, { lowS: true })).toBe(true);
		expect(p256.verify(decode(signature), digest, point, { lowS: true })).toBe(true);
	});
});
