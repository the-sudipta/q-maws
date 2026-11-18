# Benchmark data (simulated)

The simulated datasets of the extended benchmark (docs/PREREGISTRATION.md, Amendments 8, 10 and 10a on the branch `cloud-benchmark`) and the speed datasets of M13, made again on the development laptop by `scripts/cloud/make_simulated_data.sh` with IQ-TREE 2.4.0 for Windows (AliSim) and Q-MAWS 1.0.0 for Windows, the committed scripts (`scripts/simulate_benchmark.sh`, `scripts/simulate_scaling.sh`) and their seeds. AliSim gave other data on GitHub's Windows runners (Amendment 10a), so the data are stored here, as the H5 data are on the branch `h5-data`.

Each archive holds one folder `data/raw/simulated/<dataset>/` (sequences, gene trees, partitions and transfers; AliSim logs left out). The SHA-256 of the sequences of every dataset equals the value in `data/manifests/simulated/benchmark.tsv` or `scaling.tsv` (for fragmented assemblies: of all files joined in name order), and the species trees and transfer tables made again are byte for byte the committed ones. `scripts/cloud/check_simulated.sh` checks every dataset again before any run uses it.

| Archive | SHA-256 of the sequences |
|---|---|
| `benchdata/bm_div005_r1.tar.gz` | `c75c3c85821d2c2bfd2e8bc147d20ed3ba24c13504a051532966669dc1fdaa86` |
| `benchdata/bm_div005_r2.tar.gz` | `80297fec1e62699d16c9812f36a0211ba5bd8a31edac10a56016d4b80f4b6464` |
| `benchdata/bm_div005_r3.tar.gz` | `ed3da778e09a4e37a5c53909b0065a6e9689c15369890875f5f08c37ea1c0188` |
| `benchdata/bm_div005_r4.tar.gz` | `3b5bbc32b67c616d06b5ef8f8b5d7aaae9f502a00f64867381b9e2c9f069ed9c` |
| `benchdata/bm_div005_r5.tar.gz` | `66b4e260ec5961fb09fabeff676f833bc90be015c341701b24d7b11cfe1ee736` |
| `benchdata/bm_div020_r1.tar.gz` | `69a60986c4f51184ff59702321d7be331c1c2109441770acbab306372243e155` |
| `benchdata/bm_div020_r2.tar.gz` | `e33da2ba5c6d14a5e7d99f8e71fa31c6a95ddcaa8eceaf141bd0137b3054fde3` |
| `benchdata/bm_div020_r3.tar.gz` | `6ae55cfbfb9644565cad92e20bfeb1952ccc441878825c753f1130baab4a3c89` |
| `benchdata/bm_div020_r4.tar.gz` | `f765f2bdfc9bfa751c9905fadae5c82d7d9092c912f4e615a5d5e72ce4cbc729` |
| `benchdata/bm_div020_r5.tar.gz` | `c1acaecb88e1f6627a8e795d365d13b94ada5849b5a3a8da3b58fd74181b3d99` |
| `benchdata/bm_div050_r1.tar.gz` | `f83383899fc239a0b9ea8a8e0084921c15ff6c2290c3c6f9f2ae53347a8ceb92` |
| `benchdata/bm_div050_r2.tar.gz` | `8756ec35f103eeea27939f72071da3514190ca568397ea2c5bbd150b992ad81b` |
| `benchdata/bm_div050_r3.tar.gz` | `1a8a05c43eb395276cd8954e2a9e330b615357f3eaaebb3976520d13b749aea7` |
| `benchdata/bm_div050_r4.tar.gz` | `704df2fd7c3b9cf9c00c62e2ebd1f7d6b696e4233841cdc986ce696fb05a1d28` |
| `benchdata/bm_div050_r5.tar.gz` | `15e9e60c8393da42ee290f7d4bd1c597055f134140932c7d1882c5ef8d09a837` |
| `benchdata/bm_m15_r1.tar.gz` | `b7e1e1ca2cff19f7b0b36f9a62d6e36538a8f70470a5256c8faa49d5ee4d83c8` |
| `benchdata/bm_m15_r2.tar.gz` | `8a8287eed6da457841e5bf3fea12c1cde6598cf150548f73761ed7d724cb7890` |
| `benchdata/bm_m15_r3.tar.gz` | `39c64716b9c8dc4f217f7f9439fdce6466c09bd26503695214b14f1a88bb6b82` |
| `benchdata/bm_m15_r4.tar.gz` | `f622bc770c9d185b7019eab96c50f4b6f8a96aed939933d5c636e89930a9fd98` |
| `benchdata/bm_m15_r5.tar.gz` | `83ac0d81c89e44da5b489194eb67c5fb502ca37691351d74a2f4eaa1b3f6c868` |
| `benchdata/bm_m60_r1.tar.gz` | `862c47d9348f631071897abe4ffa1c39c647cfb4450ef595459073f46f2bc81a` |
| `benchdata/bm_m60_r2.tar.gz` | `7ea08e10ee6703d51ad8b6de01b7e6c09d11c94222c4d2d9697d53da1ebb05b9` |
| `benchdata/bm_m60_r3.tar.gz` | `7ffb4c6e92a732f532f080b68c726d8e562955927ba0100021176c0326fb4acb` |
| `benchdata/bm_m60_r4.tar.gz` | `faa9e60c97a38f44fe5b47192d08fa07d38db2f74f496df849136c3444757ad0` |
| `benchdata/bm_m60_r5.tar.gz` | `97270ff599be03d7b58d7b046838c944d4458ad9c13f1341c8e4cc9ead083bf4` |
| `benchdata/bm_tune_r1.tar.gz` | `9527f9e0b8d763e104d4a47bc05a8918ef6e916c206d4124de0da9c9d27d9780` |
| `benchdata/bm_tune_r2.tar.gz` | `949993f0cb8d4e33514509a68b56b43e3708b7afe497fc099ddfaf03eb65c748` |
| `benchdata/bm_tune_r3.tar.gz` | `7d2a96d81383f3153bf2c42f22dac9a7d544de3c4ea9ec1af065de221af68f01` |
| `benchdata/bm_tune_r4.tar.gz` | `720b39da213ad5681f5a87f6c0b1033f88655ae773e3c15199e705e733394d61` |
| `benchdata/bm_tune_r5.tar.gz` | `a2268401a702333d02d3e4bc9d12bbaf56de5317b05801204065d996f34ae888` |
| `benchdata/bm_tune_r6.tar.gz` | `ec7531dafe50298f24284c22ac7ee8783c79edad2f6c7f63367bee034933e1ce` |
| `benchdata/bm_frag_r1.tar.gz` | `c556bf01dfa81eba5e01620e3296e792731e367ec644d349fe2f23718da633c1` |
| `benchdata/bm_frag_r2.tar.gz` | `bc19def0cf1bffd7d51e4a3be1340d00cc50ea025aa4387575b677a7e2d0735c` |
| `benchdata/bm_frag_r3.tar.gz` | `ac01baa86acf413c78f870ff34284cc7eecc57be994e5b6197640fc675f9a1dd` |
| `benchdata/bm_frag_r4.tar.gz` | `d39f2f7481fc53354a52a298479fcac6ef55a5458b13e0a7f0f005302a53e02a` |
| `benchdata/bm_frag_r5.tar.gz` | `836e8ef0f3bfa6ad93fb8fcb91534ee9b9e6743794388c4448e7b551c57774be` |
| `benchdata/scaling_m25_r1.tar.gz` | `396d763fda2f55c7af94d6252ae1c0d32ba30bf951dcf01eab7c39073989febe` |
| `benchdata/scaling_m25_r2.tar.gz` | `e22bf4108ea626323e4a363dbf94e89c3db837f0630efb4dd30e3b71ba8dc30f` |
| `benchdata/scaling_m25_r3.tar.gz` | `a6cb716207a85e4b21bb7d269458082d29dec445c93e0fb56550e457247ff045` |
| `benchdata/scaling_m50_r1.tar.gz` | `ef4c1335718272d8c0e71fdd8cdfbeacdfcb22247c3ba0950c8d36f50a6e00e4` |
| `benchdata/scaling_m50_r2.tar.gz` | `c53af23029e3d367e940eacccde6ff9b8dcbe1734f04ffa167689b3c2ee4e665` |
| `benchdata/scaling_m50_r3.tar.gz` | `a0a00f60c80e27eb5505000635fce1505a726bb0b4467ea274c115b6951bf2dd` |
| `benchdata/scaling_m100_r1.tar.gz` | `0e2e1ad6952abef3b15c4018b0455331106ab8b6fd40519bd16a9761d7d03611` |
| `benchdata/scaling_m100_r2.tar.gz` | `33d8b3ef3343af4d7a7cdda248cae3cd977d7a5e703daadfb30606a3b07e5937` |
| `benchdata/scaling_m100_r3.tar.gz` | `65285a7577a939bf22f8284059e1be9cd77debc60cc8f499d49f9ae6759354f8` |
