(define path "/api/v2/users/123")

(cond
  ((starts-with? path "/api/v2") (print "Route to: new service"))
  ((starts-with? path "/api/v1") (print "Route to: legacy service"))
  (else (print "Route to: default")))
