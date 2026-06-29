;; Mock auth server: returns 200 if X-Api-Key is "valid-key", 403 otherwise
(define (on-request req)
  (let ((headers (table-get req "headers")))
    (let ((key (table-get headers "x-api-key")))
      (if (not key)
          (respond 403 "missing key")
          (if (string-eq? key "valid-key")
              (respond 200 "authenticated")
              (respond 403 "invalid key"))))))
