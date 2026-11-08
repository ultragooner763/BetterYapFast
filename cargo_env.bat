@echo off
set "PATH=C:\Program Files\LLVM\bin;C:\mingw64\bin;%PATH%"
set "SKIA_BINARIES_URL=file://D:/aexperiment/.local/windows/skia/prepared/skia-binaries-30669978c3f39a73e8a7-x86_64-pc-windows-msvc-d3d-ganesh-gl-jpegd-jpege-pdf-static.tar.gz"
cargo %*
