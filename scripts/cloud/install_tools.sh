#!/bin/bash
# Installs every program of the extended benchmark on a GitHub-hosted Ubuntu 24.04
# runner (docs/PREREGISTRATION.md, Amendments 8b, 9 and 10).
#
# Usage:
#   sudo bash scripts/cloud/install_tools.sh system     Ubuntu packages (every job, not cached)
#   bash scripts/cloud/install_tools.sh build           everything else into $BENCH_TOOLS (cached)
#   bash scripts/cloud/install_tools.sh versions <out>  write the version of every program
#
# Layout of $BENCH_TOOLS (default /opt/bench) is the one scripts/bench_tools.sh and
# scripts/bench_refs.sh expect. Every source is pinned below; nothing is taken
# from a moving branch. Several tools compile with -march=native, as their authors
# ship them, so the cache of a build is keyed by processor model (see the workflow).

set -euo pipefail

B=${BENCH_TOOLS:-/opt/bench}

# --- pinned sources: the only place where versions are set ---------------------
readonly QMAWS_URL=https://github.com/the-sudipta/q-maws/releases/download/v1.0.0/Q-MAWS-1.0.0-linux-x64.tar.gz
readonly QMAWS_SHA256=b88f7675e743f12f0dcb169a7d2b51461affe11409b31d491a8389cd7e991a24
readonly IQTREE_URL=https://github.com/iqtree/iqtree2/releases/download/v2.4.0/iqtree-2.4.0-Linux-intel.tar.gz
readonly IQTREE_SHA256=49c58e26f192dd5e0045110cee0af151203dfa0bd79fa03d01fcfc3e44ca2f1b
readonly MASH_URL=https://github.com/marbl/Mash/releases/download/v2.3/mash-Linux64-v2.3.tar
readonly MLMAWS=https://github.com/PapriSaha/ML-MAWS@0c38db12d9ad271aafcb4940d7558dfcd00925c1
readonly CDMAWS=https://github.com/TamimEhsan/cd-maws-sa@8bfe7d55412af0f211e969f85adf9d20f0d3af53
readonly PEAFOWL=https://github.com/hasin-abrar/Peafowl-repo@89ed477bba1072cef891b7304896891d233edd91
readonly FSWM=https://github.com/burkhard-morgenstern/FSWM@59efdc6d165c31e57eb8f63e48619121ecc8d40f
readonly MULTISPAM=https://github.com/tdencker/multi-SpaM@3230d3f586bc617f71a289813cf28624a0222046
readonly SANS=https://github.com/gi-bielefeld/sans@3d3cdd973edebfeec105ce98fb0342ce7296624f
readonly CVTREE=https://github.com/ghzuo/cvtree@575ba9be8c7ca39e95c3c19568f65791e803f3a8
readonly JOLYTREE=https://gitlab.pasteur.fr/GIPhy/JolyTree@ffbff7b9
readonly FASTME=https://gite.lirmm.fr/atgc/FastME@08e2150495640a45b89fb816edd894bec8219186
readonly REQ=https://gitlab.pasteur.fr/GIPhy/REQ@a5280404
readonly SKMER=https://github.com/shahab-sarmashghi/Skmer@ea948d3a00bbbcdb21a6faac13d4bedef2a60d48
readonly MASHTREE=https://github.com/lskatz/mashtree@c0853a86cf52ee4e47c83c6194c747cb9b9dbf5f
readonly ASTER=https://github.com/chaoszhang/ASTER@db2b3e95da5bb0318b933afe1a144eb943ef7cbf
readonly TREEQMC=https://github.com/molloy-lab/TREE-QMC@b5bfe82dbe95590d53dbc603b1073ea1fe61fd7f
readonly BIOPYTHON=1.84
readonly APT_PACKAGES="build-essential cmake git curl gawk zlib1g-dev libexpat1-dev cpanminus
    nlohmann-json3-dev python3-venv default-jdk-headless time
    jellyfish raxml andi phylonium fasttree seqtk quicktree"

log() { echo "[install $(date -u +%T)] $*"; }

# clone <repo@commit> <folder>: a full clone checked out at the pinned commit
clone() {
    local url=${1%@*} commit=${1#*@} dir=$2
    rm -rf "$dir"
    git clone -q "$url" "$dir"
    git -C "$dir" checkout -q "$commit"
}

# fetch <url> <file> <sha256 or empty>: download and check the SHA-256 when it is pinned
fetch() {
    curl -sSfL --retry 5 -o "$2" "$1"
    [ -z "$3" ] || echo "$3  $2" | sha256sum -c --quiet -
}

# --- one function per program ---------------------------------------------------
install_system() {
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -q
    # shellcheck disable=SC2086
    apt-get install -y -q --no-install-recommends $APT_PACKAGES
}

install_python_env() {  # Biopython for the neighbour-joining trees (scripts/nj_tree.py)
    python3 -m venv "$B/venv"
    "$B/venv/bin/pip" install -q "biopython==$BIOPYTHON"
}

install_qmaws() {
    fetch "$QMAWS_URL" "$B/qmaws.tar.gz" "$QMAWS_SHA256"
    mkdir -p "$B/qmaws-1.0.0"
    tar -xzf "$B/qmaws.tar.gz" -C "$B/qmaws-1.0.0" --strip-components=1
    rm "$B/qmaws.tar.gz"
    # the release bundle names the program Q-MAWS (scripts call it qmaws)
    ln -sf "$B/qmaws-1.0.0/Q-MAWS" "$B/bin/qmaws"
    "$B/bin/qmaws" --version | grep -q 1.0.0
}

install_fasttree_name() {  # bench_refs.sh calls FastTree; the Ubuntu package may only install fasttree
    command -v FastTree > /dev/null || ln -sf "$(command -v fasttree)" "$B/bin/FastTree"
}

install_iqtree() {
    fetch "$IQTREE_URL" "$B/iqtree.tar.gz" "$IQTREE_SHA256"
    tar -xzf "$B/iqtree.tar.gz" -C "$B"
    rm "$B/iqtree.tar.gz"
    ln -sf "$B/iqtree-2.4.0-Linux-intel/bin/iqtree2" "$B/bin/iqtree2"
}

install_mlmaws() {  # as for H4 (scripts/mlmaws_h4.sh)
    clone "$MLMAWS" "$B/ML-MAWS"
    (cd "$B/ML-MAWS" && g++ -O2 -std=c++17 -fopenmp -include cstdint -o ml-maws ./*.cpp)
}

install_mash() {
    fetch "$MASH_URL" "$B/mash.tar" ""
    tar -xf "$B/mash.tar" -C "$B"
    rm "$B/mash.tar"
    ln -sf "$B/mash-Linux64-v2.3/mash" "$B/bin/mash"
}

install_cdmaws() {  # flags as in scripts/baselines.sh (its own #pragma target needs them)
    clone "$CDMAWS" "$B/cd-maws-sa"
    (cd "$B/cd-maws-sa/cd-maws-sa" &&
        g++ -O2 -mavx2 -mbmi -mbmi2 -mlzcnt -o maw-gen maw-gen.cpp &&
        g++ -O2 -o dist-calc dist-calc.cpp)
}

install_peafowl() {  # Peafowl calls raxmlHPC-PTHREADS; the Ubuntu package names it by CPU feature
    clone "$PEAFOWL" "$B/Peafowl-repo"
    local rx
    rx=$(command -v raxmlHPC-PTHREADS || command -v raxmlHPC-PTHREADS-SSE3 || command -v raxmlHPC-PTHREADS-AVX2 || command -v raxmlHPC-PTHREADS-AVX)
    ln -sf "$rx" "$B/bin/raxmlHPC-PTHREADS"
    command -v raxmlHPC > /dev/null || ln -sf "$rx" "$B/bin/raxmlHPC"
}

install_fswm() {  # its Makefile, with <cstdint> supplied for current g++ (Amendment 8b)
    clone "$FSWM" "$B/FSWM"
    (cd "$B/FSWM" && g++ -O3 -std=c++11 -include cstdint Bucket.cpp Seed.cpp Fswm.cpp pattern.cpp Sequence.cpp Word.cpp -o fswm -fopenmp)
}

install_multispam() {  # CMake build (gives bin/multi-SpaM and PHYLIP's bin/consense), Amendment 8b flags
    clone "$MULTISPAM" "$B/multi-SpaM"
    (cd "$B/multi-SpaM" && cmake -S . -B build -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_C_FLAGS="-fcommon" -DCMAKE_CXX_FLAGS="-include cstdint" > /dev/null &&
        cmake --build build -j "$(nproc)" > /dev/null)
    [ -x "$B/multi-SpaM/bin/multi-SpaM" ] && [ -x "$B/multi-SpaM/bin/consense" ]
}

install_sans() {
    clone "$SANS" "$B/sans"
    make -C "$B/sans" -j "$(nproc)" > /dev/null
}

install_cvtree() {
    clone "$CVTREE" "$B/cvtree"
    (cd "$B/cvtree" && cmake -S . -B build -DCMAKE_BUILD_TYPE=Release > /dev/null &&
        cmake --build build -j "$(nproc)" > /dev/null)
}

install_fastme() {  # JolyTree's tree builder
    clone "$FASTME" "$B/FastME"
    (cd "$B/FastME" && ./configure -q > /dev/null && make -j "$(nproc)" > /dev/null)
    ln -sf "$B/FastME/src/fastme" "$B/bin/fastme"
}

install_req() {  # JolyTree checks for REQ even with -x; a Java program, run through a wrapper
    clone "$REQ" "$B/REQ"
    (cd "$B/REQ/src" && javac REQ.java && printf 'Main-Class: REQ\n' > MANIFEST.MF && jar cfm REQ.jar MANIFEST.MF ./*.class)
    printf '#!/bin/sh\nexec java -jar %s "$@"\n' "$B/REQ/src/REQ.jar" > "$B/bin/REQ"
    chmod +x "$B/bin/REQ"
}

install_jolytree() {
    install_fastme
    install_req
    clone "$JOLYTREE" "$B/JolyTree"
    chmod +x "$B/JolyTree/JolyTree.sh"
}

install_skmer() {  # needs jellyfish, mash and seqtk on PATH at run time
    clone "$SKMER" "$B/Skmer"
    python3 -m venv "$B/skmer-venv"
    "$B/skmer-venv/bin/pip" install -q "$B/Skmer"
}

install_mashtree() {  # into a local Perl library; the wrapper sets PERL5LIB
    clone "$MASHTREE" "$B/mashtree"
    cpanm -q -l "$B/perl5" --notest BioPerl Bio::Sketch::Mash > /dev/null
    (cd "$B/mashtree" && PERL5LIB="$B/perl5/lib/perl5" cpanm -q -l "$B/perl5" --notest . > /dev/null)
    printf '#!/bin/sh\nexport PERL5LIB=%s/perl5/lib/perl5${PERL5LIB:+:$PERL5LIB}\nexec %s/perl5/bin/mashtree "$@"\n' "$B" "$B" > "$B/bin/mashtree"
    chmod +x "$B/bin/mashtree"
}

install_aster() {
    clone "$ASTER" "$B/ASTER"
    make -C "$B/ASTER" caster-site > /dev/null
}

install_treeqmc() {  # the build of its README, written to build/tree-qmc (scripts/bench_refs.sh)
    clone "$TREEQMC" "$B/TREE-QMC"
    (cd "$B/TREE-QMC" && make -C external/MQLib -j "$(nproc)" > /dev/null && mkdir -p build &&
        g++ -std=c++11 -O2 -I external/MQLib/include -I external/toms743 -o build/tree-qmc \
            src/*.cpp external/toms743/toms743.cpp external/MQLib/bin/MQLib.a -lm \
            -DVERSION=\""$(cat version.txt)"\")
}

readonly STEPS="python_env qmaws iqtree mlmaws mash cdmaws peafowl fswm multispam sans cvtree
    jolytree skmer mashtree aster treeqmc fasttree_name"

# build_all: every program in its own process (so `set -e` stops only that program); a
# failure is published as its own annotation and the others still build, so one run
# shows every problem. Any failure fails the build (and nothing is cached).
build_all() {
    mkdir -p "$B/bin"
    local step failed="" out
    for step in $STEPS; do
        log "$step"
        out=$(mktemp)
        if ! bash "$0" step "$step" > "$out" 2>&1; then
            failed="$failed $step"
            cat "$out"
            echo "::error title=build $step::$(tail -n 40 "$out" | tr -d '\r' | sed -e 's/%/%25/g' | awk 'BEGIN{ORS="%0A"} {print}')"
        fi
        rm -f "$out"
    done
    [ -z "$failed" ] || { log "FAILED:$failed"; return 1; }
    log "done"
}

# versions <out>: one line per program, from the programs themselves where they tell it
versions() {
    local out=$1
    export PATH="$B/bin:$PATH"
    {
        echo -e "program\tversion"
        echo -e "qmaws\t$(qmaws --version 2>&1 | head -n 1)"
        echo -e "iqtree2\t$(iqtree2 --version 2>&1 | grep -m 1 -i version)"
        echo -e "mash\t$(mash --version)"
        local r
        for r in "$MLMAWS" "$CDMAWS" "$PEAFOWL" "$FSWM" "$MULTISPAM" "$SANS" "$CVTREE" "$JOLYTREE" \
                 "$FASTME" "$REQ" "$SKMER" "$MASHTREE" "$ASTER" "$TREEQMC"; do
            echo -e "$(basename "${r%@*}")\tcommit ${r#*@}"
        done
        echo -e "biopython\t$("$B/venv/bin/python" -c 'import Bio; print(Bio.__version__)')"
        echo -e "skmer-python\t$("$B/skmer-venv/bin/pip" freeze | tr '\n' ' ')"
        echo -e "java\t$(java -version 2>&1 | head -n 1)"
        echo -e "g++\t$(g++ --version | head -n 1)"
        dpkg-query -W -f '${Package}\t${Version} (Ubuntu package)\n' jellyfish raxml andi phylonium fasttree seqtk quicktree gawk
    } > "$out"
}

case ${1:-} in
    system) install_system ;;
    build) build_all ;;
    step) "install_${2:?program}" ;;                   # one program (used by build_all)
    qmaws) mkdir -p "$B/bin" && install_qmaws ;;      # Q-MAWS only (data and analysis jobs)
    versions) versions "${2:?output file}" ;;
    *) echo "usage: $0 system | build | step <program> | qmaws | versions <out>" >&2; exit 2 ;;
esac
