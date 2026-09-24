# Vendor-specific material stays private

Status: accepted, 2026-09-23.

Generic engines are public; anything tied to a vendor's authenticated portals,
licensed content or private evaluation questions lives in the private
`ctm-collection` repository or on the workstation. This covers the Control-M
source policy, the BMC connectors (SSO, knowledge base, EPD), the corpus and the
golden questions. Public CI never sees vendor content; gitleaks and a path check
guard the public repositories.
