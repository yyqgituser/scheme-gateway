(define n 15)

(cond
  ((= n 10) (print 10))
  ((= n 15) (print 15))
  (else (print 0)))

(print (and #t #t 42))
(print (and #t #f 42))
(print (or #f #f 99))
(print (or #f 77 99))
