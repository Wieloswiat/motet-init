import type { PageLoad } from './$types';

export const load: PageLoad = ({ url }) => ({ returnTo: url.searchParams.get('returnTo') ?? '/' });
