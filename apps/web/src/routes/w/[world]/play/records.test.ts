import { describe, expect, it } from 'vitest';
import type { at_motet_pilot_querydefs } from '@motet/lexicons';
import { decodePost, decodeScene } from './records.js';

const world = 'did:plc:world';
const scene: at_motet_pilot_querydefs.Record = {
	uri: 'at://did:plc:world/at.motet.pilot.scene/scene1',
	cid: 'bafy-scene',
	value: {
		$type: 'at.motet.pilot.scene',
		title: 'A scene',
		state: 'open',
		cast: ['did:plc:opener'],
		openedBy: 'did:plc:opener'
	}
};

const post: at_motet_pilot_querydefs.AdmittedPost = {
	admission: { uri: 'at://did:plc:world/at.motet.pilot.admission/one', cid: 'bafy-admission' },
	author: 'did:plc:opener',
	cid: 'bafy-post',
	uri: 'at://did:plc:opener/at.motet.pilot.post/one',
	place: { epoch: 0, seq: 0 },
	value: {
		$type: 'at.motet.pilot.post',
		body: 'A post',
		register: 'in-character',
		createdAt: '2026-01-01T00:00:00Z'
	}
};

describe('public scene record boundaries', () => {
	it('does not link a different World’s record as a scene in this World', () => {
		expect(decodeScene(scene, world)?.key).toBe('scene1');
		expect(decodeScene(scene, 'did:plc:other')).toBeNull();
		expect(decodeScene({ ...scene, uri: `${scene.uri}/../other` }, world)).toBeNull();
	});

	it('does not present a corrupt cast as real admission authority', () => {
		const invalid = {
			...scene,
			value: {
				$type: 'at.motet.pilot.scene',
				title: 'A scene',
				state: 'open',
				cast: ['did:plc:opener', { did: 'did:plc:someone' }],
				openedBy: 'did:plc:opener'
			}
		};
		expect(decodeScene(invalid, world)).toBeNull();
	});

	it('does not coerce a malformed post body into visible content', () => {
		expect(decodePost(post)?.body).toBe('A post');
		expect(
			decodePost({
				...post,
				value: {
					$type: 'at.motet.pilot.post',
					body: { text: 'not a string' },
					register: 'in-character',
					createdAt: '2026-01-01T00:00:00Z'
				}
			})
		).toBeNull();
	});
});
