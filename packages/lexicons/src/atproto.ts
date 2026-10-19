// The atproto data model types generated code refers to. Hand-written: these
// are the protocol's, not Motet's, and don't change with our lexicons.

/** A CID in the JSON encoding of the data model. */
export interface CidLink {
	$link: string;
}

/** A blob reference, as records store it. */
export interface Blob {
	$type: 'blob';
	ref: CidLink;
	mimeType: string;
	size: number;
}
