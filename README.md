Maker of ICE (Internal Compiler Error).

A small program to automatically find crashes in the rust compiler, clippy, rustdoc, rustfmt and miri.
Runs executable on a vast number of rust files (such as rustcs testsuit or glacier) and filters all the crashes.

Features:
* run rustc, clippy, rustdoc, rustfmt, miri or cg_clif on a file and check if there is a crash
* parallel execution
* check different combinations of RUSTFLAGS
* try to find minimal set of RUSTFLAGS that reproduces the internal compiler error
* check if a file compiles stably with incremental compilation
* build and run a file under miri

Requirements: 
 * by default, we build with the "ci" feature disabled and require "systemd-run" to limit memory and runtime duration of a process

Trophy case (200+):  
https://github.com/rust-lang/rust-clippy/issues/9463  
https://github.com/rust-lang/rust-clippy/issues/9459  
https://github.com/rust-lang/rust-clippy/issues/9433  
https://github.com/rust-lang/rust/issues/101517  
https://github.com/rust-lang/rust/issues/101505  
https://github.com/rust-lang/rust/issues/101243  
https://github.com/rust-lang/rust/issues/101113  
https://github.com/rust-lang/rust/issues/101076  
https://github.com/rust-lang/rust/issues/100948  
https://github.com/rust-lang/miri/issues/2499  
https://github.com/rust-lang/rust/issues/100783  
https://github.com/rust-lang/rust/issues/100778  
https://github.com/rust-lang/rust/issues/100772  
https://github.com/rust-lang/rust/issues/100770  
https://github.com/rust-lang/miri/issues/2496  
https://github.com/rust-lang/rust/issues/100612  
https://github.com/rust-lang/rust/issues/100485  
https://github.com/rust-lang/rust/issues/100484  
https://github.com/rust-lang/rust/issues/100191  
https://github.com/rust-lang/rust/issues/100187  
https://github.com/rust-lang/rust/issues/100154  
https://github.com/rust-lang/rust/issues/100047  
https://github.com/rust-lang/rust/issues/99876  
https://github.com/rust-lang/rust/issues/99820  
https://github.com/rust-lang/miri/issues/2433  
https://github.com/rust-lang/miri/issues/2432  
https://github.com/rust-lang/rust/issues/99662  
https://github.com/rust-lang/rust/issues/99647  
https://github.com/rust-lang/rust/issues/99387  
https://github.com/rust-lang/rust/issues/99363  
https://github.com/rust-lang/rust/issues/99331  
https://github.com/rust-lang/rust/issues/99325  
https://github.com/rust-lang/rust/issues/99319  
https://github.com/rust-lang/rust/issues/99318  
https://github.com/rust-lang/rust/issues/99228  
https://github.com/bjorn3/rustc_codegen_cranelift/issues/1244  
https://github.com/bjorn3/rustc_codegen_cranelift/issues/1243  
https://github.com/bjorn3/rustc_codegen_cranelift/issues/1242  
https://github.com/bjorn3/rustc_codegen_cranelift/issues/1241  
https://github.com/bjorn3/rustc_codegen_cranelift/issues/1240  
https://github.com/bjorn3/rustc_codegen_cranelift/issues/1239  
https://github.com/bjorn3/rustc_codegen_cranelift/issues/1238  
https://github.com/bjorn3/rustc_codegen_cranelift/issues/1237  
https://github.com/bjorn3/rustc_codegen_cranelift/issues/1236  
https://github.com/bjorn3/rustc_codegen_cranelift/issues/1235  
https://github.com/rust-lang/miri/issues/2340  
https://github.com/rust-lang/rust/issues/98821  
https://github.com/rust-lang/rust/issues/98372  
https://github.com/rust-lang/rust/issues/98250  
https://github.com/rust-lang/rust/issues/98171  
https://github.com/rust-lang/miri/issues/2234  
https://github.com/rust-lang/rust/issues/98016  
https://github.com/rust-lang/rust/issues/98009  
https://github.com/rust-lang/rust/issues/98007  
https://github.com/rust-lang/rust/issues/98006  
https://github.com/rust-lang/rust/issues/98003  
https://github.com/rust-lang/rust/issues/98002  
https://github.com/rust-lang/rust/issues/97728  
https://github.com/rust-lang/rust/issues/97726  
https://github.com/rust-lang/rust/issues/97725  
https://github.com/rust-lang/rust/issues/97698  
https://github.com/rust-lang/rust/issues/97695  
https://github.com/rust-lang/rust/issues/97270  
https://github.com/rust-lang/rust/issues/97194  
https://github.com/rust-lang/rust/issues/97193  
https://github.com/rust-lang/rust/issues/97049  
https://github.com/rust-lang/rust/issues/97006  
https://github.com/rust-lang/miri/issues/2106  
https://github.com/rust-lang/miri/issues/2088  
https://github.com/rust-lang/rust/issues/96765  
https://github.com/rust-lang/rust/issues/96767  
https://github.com/rust-lang/rust/issues/96721  
https://github.com/rust-lang/rust/issues/96485  
https://github.com/rust-lang/rust/issues/96395  
https://github.com/rust-lang/rust-clippy/issues/8748  
https://github.com/rust-lang/rust/issues/96381  
https://github.com/rust-lang/rust/issues/96288  
https://github.com/rust-lang/rust/issues/96287  
https://github.com/rust-lang/rust/issues/96200  
https://github.com/rust-lang/rust/issues/96191  
https://github.com/rust-lang/rust/issues/96185  
https://github.com/rust-lang/rust/issues/96175  
https://github.com/rust-lang/rust/issues/96172  
https://github.com/rust-lang/rust/issues/96170  
https://github.com/rust-lang/rust/issues/96169  
https://github.com/rust-lang/rust/issues/96114  
https://github.com/rust-lang/rust/issues/95978  
https://github.com/rust-lang/rust/issues/95879  
https://github.com/rust-lang/rust/issues/95878  
https://github.com/rust-lang/rust/issues/95640  
https://github.com/rust-lang/rust/issues/95327  
https://github.com/rust-lang/rust/issues/95307  
https://github.com/rust-lang/rust/issues/95163  
https://github.com/rust-lang/rust/issues/95128  
https://github.com/rust-lang/rust/issues/95023  
https://github.com/rust-lang/rust/issues/94961  
https://github.com/rust-lang/rust/issues/94953  
https://github.com/rust-lang/rust/issues/94903  
https://github.com/rust-lang/rust/issues/94822  
https://github.com/rust-lang/rust/issues/94725  
https://github.com/rust-lang/rust/issues/94654  
https://github.com/rust-lang/rust/issues/94629  
https://github.com/rust-lang/rust/issues/94627  
https://github.com/rust-lang/rust/issues/94516  
https://github.com/rust-lang/rust/issues/94382  
https://github.com/rust-lang/rust/issues/94380  
https://github.com/rust-lang/rust/issues/94379  
https://github.com/rust-lang/rust/issues/94378  
https://github.com/rust-lang/rust/issues/94171  
https://github.com/rust-lang/rust/issues/94149  
https://github.com/rust-lang/rust/issues/94073  
https://github.com/rust-lang/rust/issues/93871  
https://github.com/rust-lang/rust/issues/93788  
https://github.com/rust-lang/rust/issues/93688  
https://github.com/rust-lang/rust/issues/93578  
https://github.com/rust-lang/rust/issues/93117  
https://github.com/rust-lang/rust-clippy/issues/8245  
https://github.com/rust-lang/rust-clippy/issues/8244  
https://github.com/rust-lang/rust/issues/92495  
https://github.com/rust-lang/rust/issues/92240  
https://github.com/rust-lang/rust/issues/91745  
https://github.com/rust-lang/rust/issues/90192  
https://github.com/rust-lang/rust/issues/90191  
https://github.com/rust-lang/rust/issues/90189  
https://github.com/rust-lang/rust/issues/89312  
https://github.com/rust-lang/rust/issues/89271  
https://github.com/rust-lang/rust/issues/89066  
https://github.com/rust-lang/rust/issues/88536  
https://github.com/rust-lang/rustfmt/issues/4968  
https://github.com/rust-lang/rust/issues/88434  
https://github.com/rust-lang/rust/issues/88433  
https://github.com/rust-lang/rust/issues/88171  
https://github.com/rust-lang/rust/issues/87563  
https://github.com/rust-lang/rust/issues/87308  
https://github.com/rust-lang/rust/issues/87219  
https://github.com/rust-lang/rust/issues/87218  
https://github.com/rust-lang/rust/issues/85871  
https://github.com/rust-lang/rust/issues/85552  
https://github.com/rust-lang/rust/issues/85480  
https://github.com/rust-lang/rust/issues/83921  
https://github.com/rust-lang/rust/issues/83190  
https://github.com/rust-lang/rust/issues/83048  
https://github.com/rust-lang/rust/issues/82678  
https://github.com/rust-lang/rust/issues/82329  
https://github.com/rust-lang/rust/issues/82328  
https://github.com/rust-lang/rust/issues/82327  
https://github.com/rust-lang/rust/issues/82326  
https://github.com/rust-lang/rust/issues/82325  
https://github.com/rust-lang/rust/issues/81627  
https://github.com/rust-lang/rust/issues/81403  
https://github.com/rust-lang/rust/issues/80589  
https://github.com/rust-lang/rust/issues/80251  
https://github.com/rust-lang/rust/issues/80231  
https://github.com/rust-lang/rust/issues/80230  
https://github.com/rust-lang/rust/issues/80229  
https://github.com/rust-lang/rust/issues/80228  
https://github.com/rust-lang/rust/issues/80060  
https://github.com/rust-lang/rustfmt/issues/4587  
https://github.com/rust-lang/rustfmt/issues/4586  
https://github.com/rust-lang/rust/issues/79699  
https://github.com/rust-lang/rust/issues/79669  
https://github.com/rust-lang/rust/issues/79569  
https://github.com/rust-lang/rust/issues/79566  
https://github.com/rust-lang/rust/issues/79565  
https://github.com/rust-lang/rust/issues/79497  
https://github.com/rust-lang/rust/issues/79496  
https://github.com/rust-lang/rust/issues/79495  
https://github.com/rust-lang/rust/issues/79494  
https://github.com/rust-lang/rust/issues/79468  
https://github.com/rust-lang/rust/issues/79467  
https://github.com/rust-lang/rust/issues/79466  
https://github.com/rust-lang/rust/issues/79465  
https://github.com/rust-lang/rust/issues/79461  
https://github.com/rust-lang/rust/issues/79099  
https://github.com/rust-lang/rust/issues/79066  
https://github.com/rust-lang/rust/issues/78628  
https://github.com/rust-lang/rust/issues/78560  
https://github.com/rust-lang/rust/issues/78520  
https://github.com/rust-lang/rust/issues/78510  
https://github.com/rust-lang/rust/issues/78442  
https://github.com/rust-lang/rust/issues/78441  
https://github.com/rust-lang/rust/issues/78233  
https://github.com/rust-lang/rust/issues/78180  
https://github.com/rust-lang/rust/issues/77669  
https://github.com/rust-lang/rust/issues/77668  
https://github.com/rust-lang/rust/issues/75962  
https://github.com/rust-lang/rust/issues/75507  
https://github.com/rust-lang/rust/issues/75506  
https://github.com/rust-lang/rust/issues/75053  
https://github.com/rust-lang/rust/issues/75051  
https://github.com/rust-lang/rust/issues/73860  
https://github.com/rust-lang/rust/issues/74358  
https://github.com/rust-lang/rust/issues/73260  
https://github.com/rust-lang/rust/issues/73022  
https://github.com/rust-lang/rust/issues/73021  
https://github.com/rust-lang/rust/issues/73020  
https://github.com/rust-lang/rust/issues/72960  
https://github.com/rust-lang/rust/issues/72911  
https://github.com/rust-lang/rust/issues/72679  
https://github.com/rust-lang/rust/issues/72285  
https://github.com/rust-lang/rust/issues/72181  
https://github.com/rust-lang/rust/issues/72105  
https://github.com/rust-lang/rust/issues/69875  
https://github.com/rust-lang/rust/issues/69416  
https://github.com/rust-lang/rust/issues/69415  
https://github.com/rust-lang/rust/issues/69409  
https://github.com/rust-lang/rust/issues/69414  
https://github.com/rust-lang/rust/issues/69398  
https://github.com/rust-lang/rust/issues/68750  
https://github.com/rust-lang/rust/issues/68749  
https://github.com/rust-lang/rust/issues/68296  
https://github.com/rust-lang/rust/issues/67696  
https://github.com/rust-lang/rust/issues/67641  
https://github.com/rust-lang/rust/issues/67640  
https://github.com/rust-lang/rust/issues/67639  
https://github.com/rust-lang/rust/issues/67550  

#### License:

Copyright 2020-2022 Matthias Krüger

````
Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
<LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
option. All files in the project carrying such notice may not be
copied, modified, or distributed except according to those terms.
````
