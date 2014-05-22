#!/bin/sh

set -e

fetch_repo() {
    name=$(basename $1)
    if test -d $name
    then
        cd $name
        git pull
        git submodule update
        cd ..
    else
        git clone $1
    fi
}

build() {
    cd $(basename $1)
    if test -f Makefile
    then
        make clean 
    fi
    if test -f ./configure
    then
        ./configure
    fi
    make
    find -name "*.rlib" -exec cp "{}" ../build ";"
    cd ..
}

repos="https://github.com/DaGenix/rust-crypto"

mkdir -p build
for repo in $repos
do 
    fetch_repo $repo
    build $repo
done
