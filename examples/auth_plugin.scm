;; Auth plugin configuration
(define config (table
  ("auth-server-url" "http://localhost:9000/auth")
  ("auth-header-name" "x-api-key")))

;; Plugin logic: called for every HTTP request
(define (on-request req)
  (let ((header-name  (table-get config "auth-header-name"))
        (auth-url     (table-get config "auth-server-url"))
        (headers      (table-get req "headers"))
        (path         (table-get req "path")))
    (let ((header-value (table-get headers header-name)))
      (if (not header-value)
          (respond 401 "Missing auth header")
          (let ((auth-resp (http-get auth-url (table (header-name header-value)))))
            (if (= (table-get auth-resp "status") 200)
                (respond 200 (string-append "OK: " path " authorized"))
                (respond 403 "Forbidden: auth server rejected")))))))
