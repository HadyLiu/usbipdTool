# UsbipdTool 交叉编译镜像：Linux 上编译 Windows x64 可执行文件（x86_64-pc-windows-gnu）
FROM rust:1-bookworm

RUN apt-get update \
 && apt-get install -y --no-install-recommends mingw-w64 \
 && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-pc-windows-gnu

# mingw 链接器 / 资源编译器（embed-resource 交叉编译时自动使用 x86_64-w64-mingw32-windres）
ENV CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
    CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc \
    AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar

WORKDIR /src
