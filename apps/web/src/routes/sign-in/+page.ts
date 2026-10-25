import { signIn } from '#lib/demo/screens/sign-in.js';
import type { PageLoad } from './$types';

export const load: PageLoad = () => ({ screen: signIn });
