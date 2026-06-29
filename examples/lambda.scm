(define doubler (lambda (x) (* x 2)))
(print (doubler 21))
(define (apply-twice f x) (f (f x)))
(print (apply-twice doubler 3))
