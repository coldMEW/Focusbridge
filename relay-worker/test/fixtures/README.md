# Synthetic signing fixture

`cert.pem` and `key.pem` are generated exclusively for local tests. The key is
NOT a Firebase, Cloudflare, user, enrollment or application secret. Tests trust
this certificate by intercepting only the fixed Google public-certificate URL.
Production has no configurable certificate URL or fixture binding.
