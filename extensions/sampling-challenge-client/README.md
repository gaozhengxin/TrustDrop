# Sampling challenge client

Shared seller-side client for sampling-proof extensions. It:

1. derives a domain-separated key from chain ID, sale contract, sale ID, and
   proof domain;
2. signs and sends `requestSeed` to `SamplingChallengeVRFMock`;
3. waits for the receipt;
4. reads `latestChallenges(seller, key)` back from the contract; and
5. returns the checked seed and request evidence to the proof pipeline.

Use `VIDEO_SAMPLING_DOMAIN` for the video extension and
`FLOW_GRAPH_SAMPLING_DOMAIN` for the flow-graph extension. New proof types must
define a new stable domain string.

The mock contract fulfills synchronously from block-derived values. It is useful
for the hackathon interaction model but is not a production VRF: block producers
can influence the seed, and no bond or non-response penalty is implemented.
