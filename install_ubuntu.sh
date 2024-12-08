#!/bin/bash

wget -O- https://apt.llvm.org/llvm-snapshot.gpg.key | sudo apt-key add -
sudo add-apt-repository "deb http://apt.llvm.org/jammy/ llvm-toolchain-jammy-18 main" -y

sudo apt-get update

sudo apt-get install -y build-essential
sudo apt-get install -y update-alternatives
sudo apt-get install -y clang-18 llvm-18 libllvm18 llvm-18-dev llvm-18-runtime

sudo apt-get install -y libpolly-18-dev
sudo apt-get install -y libzstd-dev
sudo apt-get install -y zlib1g-dev
