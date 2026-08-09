-- Older retention runs cleared request metadata but left the request-body
-- fingerprint intact. Replace it with a deterministic, per-ledger value that
-- is independent of request content while retaining the 64-character schema
-- invariant.
UPDATE modelport_gateway_requests
SET request_fingerprint = encode(
    sha256(
        convert_to(
            'modelport-retained-request-fingerprint-v1:' || ledger_id,
            'UTF8'
        )
    ),
    'hex'
)
WHERE request_id LIKE 'retained:%'
  AND request_fingerprint <> encode(
      sha256(
          convert_to(
              'modelport-retained-request-fingerprint-v1:' || ledger_id,
              'UTF8'
          )
      ),
      'hex'
  );
