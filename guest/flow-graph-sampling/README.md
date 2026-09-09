# Authenticated flow-graph sampling guest

The Walrus blob uses a small authenticated random-access envelope: a fixed
header, one fixed-width entry per minute bucket, then contiguous NDJSON bucket
payloads. The guest authenticates the directory, derives the challenged minute
from the seed, obtains the byte range from that directory, and requires the
witness to contain exactly the primary symbols covering the directory and the
selected payload.

Every supplied symbol is verified through a primary-tree multiproof and its
sliver-pair path to the Walrus metadata root. The guest then reconstructs the
complete selected bucket, checks that it ends on an NDJSON row boundary, and
commits its CID digest with the origin blob ID, seed, and selected time
interval. Semantic inspection of the disclosed sample belongs to the buyer-side
viewer; repeating JSON parsing inside the proof would not strengthen the byte
range's binding to the origin blob.
