export class XrpcError extends Error {
	constructor(
		public readonly status: number,
		public readonly error: string,
		message: string
	) {
		super(message);
		this.name = 'XrpcError';
	}
}

async function responseBody<Output>(response: Response): Promise<Output> {
	if (!response.ok) {
		let body: { error?: string; message?: string } | null = null;

		try {
			body = await response.json();
		} catch {
			// A proxy may return an error page instead of an XRPC error.
		}

		throw new XrpcError(
			response.status,
			body?.error ?? 'RequestFailed',
			body?.message ?? `AppView request failed (${response.status})`
		);
	}

	// Output is the generated type for the endpoint supplied by the caller.
	const body: Output = await response.json();

	return body;
}

export async function query<Output>(
	fetcher: typeof fetch,
	method: string,
	params: Record<string, string | number | undefined> = {}
): Promise<Output> {
	const search = new URLSearchParams();

	for (const [name, value] of Object.entries(params)) {
		if (value !== undefined) search.set(name, String(value));
	}

	const suffix = search.size === 0 ? '' : `?${search}`;

	const response = await fetcher(`/xrpc/${method}${suffix}`, {
		headers: { accept: 'application/json' }
	});

	return responseBody<Output>(response);
}

export async function procedure<Input, Output>(method: string, input: Input): Promise<Output> {
	const response = await fetch(`/xrpc/${method}`, {
		method: 'POST',
		headers: { 'content-type': 'application/json', accept: 'application/json' },
		body: JSON.stringify(input)
	});

	return responseBody<Output>(response);
}
