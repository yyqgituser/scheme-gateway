(define config (table
  ("auth-server-url" "http://localhost:9000/auth")
  ("auth-header-name" "X-Api-Key")
  ("rate-limit" 100)))

(print config)
(print (table-get config "auth-header-name"))
(print (table-get config "rate-limit"))
(print (table-has? config "auth-server-url"))
(print (table-has? config "missing"))

(table-set! config "version" 3)
(print (table-get config "version"))

(print (table-keys config))

;; nested table
(define req (table
  ("method" "GET")
  ("path" "/api/users")
  ("headers" (table
    ("X-Api-Key" "secret123")
    ("Host" "localhost")))))

(print (table-get req "path"))
(print (table-get (table-get req "headers") "X-Api-Key"))
