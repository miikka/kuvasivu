run:
     cargo run

test-cov:
     cargo llvm-cov --html

open-cov:
     open target/llvm-cov/html/index.html
