import {
	at_motet_pilot_post,
	at_motet_pilot_scene,
	type at_motet_pilot_querydefs
} from '@motet/lexicons';

export interface SceneView {
	key: string;
	title: string;
	state: string;
	cast: string[];
	openedBy: string;
}

export interface PostView {
	body: string;
	register: string;
	createdAt: string;
}

// The field checks below are the boundary for untrusted record JSON.
/* oxlint-disable anti-slop/no-runtime-typeof */
function isString(value: unknown): value is string {
	return typeof value === 'string';
}
/* oxlint-enable anti-slop/no-runtime-typeof */
export function isSceneKey(key: string): boolean {
	return !!key && key.length <= 512 && key !== '.' && key !== '..' && /^[A-Za-z0-9._~:-]+$/.test(key);
}


export function decodeScene(record: at_motet_pilot_querydefs.Record, worldDid: string): SceneView | null {
	const prefix = `at://${worldDid}/${at_motet_pilot_scene.SceneNsid}/`;
	if (!record.uri.startsWith(prefix)) return null;
	const key = record.uri.slice(prefix.length);
	if (!isSceneKey(key)) return null;
	const value = record.value;
	/* oxlint-disable anti-slop/no-runtime-typeof */
	if (
		typeof value !== 'object' ||
		value === null ||
		Array.isArray(value) ||
		!('$type' in value) ||
		value.$type !== at_motet_pilot_scene.SceneNsid ||
		!('title' in value) ||
		!isString(value.title) ||
		!('state' in value) ||
		!isString(value.state) ||
		!('openedBy' in value) ||
		!isString(value.openedBy) ||
		!('cast' in value) ||
		!Array.isArray(value.cast) ||
		!value.cast.every(isString)
	)
		return null;
	/* oxlint-enable anti-slop/no-runtime-typeof */

	return { key, title: value.title, state: value.state, cast: value.cast, openedBy: value.openedBy };
}

export function decodePost(post: at_motet_pilot_querydefs.AdmittedPost): PostView | null {
	const value = post.value;
	/* oxlint-disable anti-slop/no-runtime-typeof */
	if (
		typeof value !== 'object' ||
		value === null ||
		Array.isArray(value) ||
		!('$type' in value) ||
		value.$type !== at_motet_pilot_post.PostNsid ||
		!('body' in value) ||
		!isString(value.body) ||
		!('register' in value) ||
		!isString(value.register) ||
		!('createdAt' in value) ||
		!isString(value.createdAt)
	)
		return null;
	/* oxlint-enable anti-slop/no-runtime-typeof */
	return { body: value.body, register: value.register, createdAt: value.createdAt };
}
