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
rust-lang/rust-clippy#9463  
rust-lang/rust-clippy#9459  
rust-lang/rust-clippy#9433  
rust-lang/rust#101517  
rust-lang/rust#101505  
rust-lang/rust#101243  
rust-lang/rust#101113  
rust-lang/rust#101076  
rust-lang/rust#100948  
rust-lang/miri#2499  
rust-lang/rust#100783  
rust-lang/rust#100778  
rust-lang/rust#100772  
rust-lang/rust#100770  
rust-lang/miri#2496  
rust-lang/rust#100612  
rust-lang/rust#100485  
rust-lang/rust#100484  
rust-lang/rust#100191  
rust-lang/rust#100187  
rust-lang/rust#100154  
rust-lang/rust#100047  
rust-lang/rust#99876  
rust-lang/rust#99820  
rust-lang/miri#2433  
rust-lang/miri#2432  
rust-lang/rust#99662  
rust-lang/rust#99647  
rust-lang/rust#99387  
rust-lang/rust#99363  
rust-lang/rust#99331  
rust-lang/rust#99325  
rust-lang/rust#99319  
rust-lang/rust#99318  
rust-lang/rust#99228  
bjorn3/rustc_codegen_cranelift#1244  
bjorn3/rustc_codegen_cranelift#1243  
bjorn3/rustc_codegen_cranelift#1242  
bjorn3/rustc_codegen_cranelift#1241  
bjorn3/rustc_codegen_cranelift#1240  
bjorn3/rustc_codegen_cranelift#1239  
bjorn3/rustc_codegen_cranelift#1238  
bjorn3/rustc_codegen_cranelift#1237  
bjorn3/rustc_codegen_cranelift#1236  
bjorn3/rustc_codegen_cranelift#1235  
rust-lang/miri#2340  
rust-lang/rust#98821  
rust-lang/rust#98372  
rust-lang/rust#98250  
rust-lang/rust#98171  
rust-lang/miri#2234  
rust-lang/rust#98016  
rust-lang/rust#98009  
rust-lang/rust#98007  
rust-lang/rust#98006  
rust-lang/rust#98003  
rust-lang/rust#98002  
rust-lang/rust#97728  
rust-lang/rust#97726  
rust-lang/rust#97725  
rust-lang/rust#97698  
rust-lang/rust#97695  
rust-lang/rust#97270  
rust-lang/rust#97194  
rust-lang/rust#97193  
rust-lang/rust#97049  
rust-lang/rust#97006  
rust-lang/miri#2106  
rust-lang/miri#2088  
rust-lang/rust#96765  
rust-lang/rust#96767  
rust-lang/rust#96721  
rust-lang/rust#96485  
rust-lang/rust#96395  
rust-lang/rust-clippy#8748  
rust-lang/rust#96381  
rust-lang/rust#96288  
rust-lang/rust#96287  
rust-lang/rust#96200  
rust-lang/rust#96191  
rust-lang/rust#96185  
rust-lang/rust#96175  
rust-lang/rust#96172  
rust-lang/rust#96170  
rust-lang/rust#96169  
rust-lang/rust#96114  
rust-lang/rust#95978  
rust-lang/rust#95879  
rust-lang/rust#95878  
rust-lang/rust#95640  
rust-lang/rust#95327  
rust-lang/rust#95307  
rust-lang/rust#95163  
rust-lang/rust#95128  
rust-lang/rust#95023  
rust-lang/rust#94961  
rust-lang/rust#94953  
rust-lang/rust#94903  
rust-lang/rust#94822  
rust-lang/rust#94725  
rust-lang/rust#94654  
rust-lang/rust#94629  
rust-lang/rust#94627  
rust-lang/rust#94516  
rust-lang/rust#94382  
rust-lang/rust#94380  
rust-lang/rust#94379  
rust-lang/rust#94378  
rust-lang/rust#94171  
rust-lang/rust#94149  
rust-lang/rust#94073  
rust-lang/rust#93871  
rust-lang/rust#93788  
rust-lang/rust#93688  
rust-lang/rust#93578  
rust-lang/rust#93117  
rust-lang/rust-clippy#8245  
rust-lang/rust-clippy#8244  
rust-lang/rust#92495  
rust-lang/rust#92240  
rust-lang/rust#91745  
rust-lang/rust#90192  
rust-lang/rust#90191  
rust-lang/rust#90189  
rust-lang/rust#89312  
rust-lang/rust#89271  
rust-lang/rust#89066  
rust-lang/rust#88536  
rust-lang/rustfmt#4968  
rust-lang/rust#88434  
rust-lang/rust#88433  
rust-lang/rust#88171  
rust-lang/rust#87563  
rust-lang/rust#87308  
rust-lang/rust#87219  
rust-lang/rust#87218  
rust-lang/rust#85871  
rust-lang/rust#85552  
rust-lang/rust#85480  
rust-lang/rust#83921  
rust-lang/rust#83190  
rust-lang/rust#83048  
rust-lang/rust#82678  
rust-lang/rust#82329  
rust-lang/rust#82328  
rust-lang/rust#82327  
rust-lang/rust#82326  
rust-lang/rust#82325  
rust-lang/rust#81627  
rust-lang/rust#81403  
rust-lang/rust#80589  
rust-lang/rust#80251  
rust-lang/rust#80231  
rust-lang/rust#80230  
rust-lang/rust#80229  
rust-lang/rust#80228  
rust-lang/rust#80060  
rust-lang/rustfmt#4587  
rust-lang/rustfmt#4586  
rust-lang/rust#79699  
rust-lang/rust#79669  
rust-lang/rust#79569  
rust-lang/rust#79566  
rust-lang/rust#79565  
rust-lang/rust#79497  
rust-lang/rust#79496  
rust-lang/rust#79495  
rust-lang/rust#79494  
rust-lang/rust#79468  
rust-lang/rust#79467  
rust-lang/rust#79466  
rust-lang/rust#79465  
rust-lang/rust#79461  
rust-lang/rust#79099  
rust-lang/rust#79066  
rust-lang/rust#78628  
rust-lang/rust#78560  
rust-lang/rust#78520  
rust-lang/rust#78510  
rust-lang/rust#78442  
rust-lang/rust#78441  
rust-lang/rust#78233  
rust-lang/rust#78180  
rust-lang/rust#77669  
rust-lang/rust#77668  
rust-lang/rust#75962  
rust-lang/rust#75507  
rust-lang/rust#75506  
rust-lang/rust#75053  
rust-lang/rust#75051  
rust-lang/rust#73860  
rust-lang/rust#74358  
rust-lang/rust#73260  
rust-lang/rust#73022  
rust-lang/rust#73021  
rust-lang/rust#73020  
rust-lang/rust#72960  
rust-lang/rust#72911  
rust-lang/rust#72679  
rust-lang/rust#72285  
rust-lang/rust#72181  
rust-lang/rust#72105  
rust-lang/rust#69875  
rust-lang/rust#69416  
rust-lang/rust#69415  
rust-lang/rust#69409  
rust-lang/rust#69414  
rust-lang/rust#69398  
rust-lang/rust#68750  
rust-lang/rust#68749  
rust-lang/rust#68296  
rust-lang/rust#67696  
rust-lang/rust#67641  
rust-lang/rust#67640  
rust-lang/rust#67639  
rust-lang/rust#67550  


#### License:

Copyright 2020-2022 Matthias Krüger

````
Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
<LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
option. All files in the project carrying such notice may not be
copied, modified, or distributed except according to those terms.
````
