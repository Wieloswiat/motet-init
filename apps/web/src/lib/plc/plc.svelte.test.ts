import { encode } from '@ipld/dag-cbor';
import { describe, expect, it, vi } from 'vitest';
import { didKey, generate, sign } from '../owner-key/index';
import { didOfSignedGenesis, signGenesis } from './index';

const handle = 'world.motet.test';
const tenor = 'https://tenor.motet.test';

function base64url(bytes: Uint8Array): string {
	let binary = '';

	for (const byte of bytes) binary += String.fromCharCode(byte);

	return btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, '');
}

async function reservation() {
	const owner = await generate();
	const key = await didKey(owner.publicKey);

	const operation = {
		type: 'plc_operation',
		prev: null,
		rotationKeys: [key],
		alsoKnownAs: [`at://${handle}`],
		verificationMethods: { atproto: key },
		services: { atproto_pds: { type: 'AtprotoPersonalDataServer', endpoint: tenor } }
	};

	return { owner, key, operation };
}

describe('PLC genesis signing boundary', () => {
	it('signs an owner-controlled genesis, so a valid reservation can create a World', async () => {
		const { owner, key, operation } = await reservation();
		const signer = vi.fn((bytes: Uint8Array<ArrayBuffer>) => sign(owner.privateKey, bytes));

		const result = await signGenesis(
			operation,
			base64url(encode(operation)),
			key,
			handle,
			`${tenor}/`,
			signer
		);

		expect(signer).toHaveBeenCalledOnce();
		expect(result.signedOperation).toMatchObject(operation);
		expect(result.signedOperation.sig).toMatch(/^[A-Za-z0-9_-]+$/);
		expect(result.did).toMatch(/^did:plc:[a-z2-7]{24}$/);
	});

	it('refuses another rotation key before signing, preventing the tenor from taking recovery control', async () => {
		const { key, operation } = await reservation();

		operation.rotationKeys.push(await didKey((await generate()).publicKey));

		const signer = vi.fn(async () => 'unused');
		await expect(
			signGenesis(operation, base64url(encode(operation)), key, handle, tenor, signer)
		).rejects.toThrow();

		expect(signer).not.toHaveBeenCalled();
	});

	it('refuses another PDS before signing, preventing an attacker from redirecting the World', async () => {
		const { key, operation } = await reservation();

		operation.services.atproto_pds.endpoint = 'https://attacker.example';

		const signer = vi.fn(async () => 'unused');
		await expect(
			signGenesis(operation, base64url(encode(operation)), key, handle, tenor, signer)
		).rejects.toThrow();

		expect(signer).not.toHaveBeenCalled();
	});

	it('refuses an altered alias before signing, preventing substitution of the requested handle', async () => {
		const { key, operation } = await reservation();

		operation.alsoKnownAs = ['at://attacker.example'];

		const signer = vi.fn(async () => 'unused');
		await expect(
			signGenesis(operation, base64url(encode(operation)), key, handle, tenor, signer)
		).rejects.toThrow();

		expect(signer).not.toHaveBeenCalled();
	});

	it('refuses bytes for a different operation before signing, preventing a benign display hiding malicious authority', async () => {
		const { key, operation } = await reservation();
		const malicious = structuredClone(operation);

		malicious.rotationKeys.push(await didKey((await generate()).publicKey));

		const signer = vi.fn(async () => 'unused');
		await expect(
			signGenesis(operation, base64url(encode(malicious)), key, handle, tenor, signer)
		).rejects.toThrow();

		expect(signer).not.toHaveBeenCalled();
	});

	it('refuses a malformed signing key before signing, preventing an unusable World identity', async () => {
		const { key, operation } = await reservation();

		operation.verificationMethods.atproto = 'did:key:znotapublickey';

		const signer = vi.fn(async () => 'unused');
		await expect(
			signGenesis(operation, base64url(encode(operation)), key, handle, tenor, signer)
		).rejects.toThrow();

		expect(signer).not.toHaveBeenCalled();
	});

	it('refuses updates and other operation types, preventing a signature on a non-genesis PLC operation', async () => {
		const { key, operation } = await reservation();
		const signer = vi.fn(async () => 'unused');

		for (const changed of [
			{ ...operation, prev: 'bafyreiaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' },
			{ ...operation, type: 'plc_tombstone' },
			{ ...operation, sig: 'existing' }
		]) {
			await expect(
				signGenesis(changed, base64url(encode(changed)), key, handle, tenor, signer)
			).rejects.toThrow();
		}

		expect(signer).not.toHaveBeenCalled();
	});

	it('derives the Rust identity fixture DID, preventing a wrong CBOR or base32 identity', async () => {
		// atproto-identity tests/plc_log_belongs_to_did.rs checks this directory genesis and CID.
		const operation = {
			sig: 'Z234aGNHJI47gYaVKtacOPnY46PMggQ-hkaMgW6GMlkXd3RCgH4sp8CJbkfjO1agIy8LK2Z0fHVzHzhWvAVwsA',
			prev: null,
			type: 'plc_operation',
			services: {
				atproto_pds: { type: 'AtprotoPersonalDataServer', endpoint: 'https://bsky.social' }
			},
			alsoKnownAs: ['at://ngerakines.bsky.social'],
			rotationKeys: [
				'did:key:zQ3shhCGUqDKjStzuDxPkTxN6ujddP4RkEKJJouJGRRkaLGbg',
				'did:key:zQ3shP5TBe1sQfSttXty15FAEHV1DZgcxRZNxvEWnPfLFwLxJ'
			],
			verificationMethods: { atproto: 'did:key:zQ3shP5TBe1sQfSttXty15FAEHV1DZgcxRZNxvEWnPfLFwLxJ' }
		};

		expect(await didOfSignedGenesis(operation)).toBe('did:plc:cbkjy5n7bk3ax2wplmtjofq2');
	});
});
