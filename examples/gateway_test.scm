(define (on-request req)
  (let ((path (table-get req "path"))
        (method (table-get req "method")))
    (cond
      ((starts-with? path "/hello")
       (respond 200 (string-append "Hello from tiny-scheme! path=" path)))
      ((starts-with? path "/api")
       (respond 200 (string-append method " " path " -> API OK")))
      (else
       (respond 404 "Not Found")))))
