Name:           garbar
Version:        0.1.0
Release:        1%{?dist}
Summary:        Configurable status bar for the gar desktop suite

License:        MIT
URL:            https://github.com/gardesk/garbar
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  rust >= 1.75
BuildRequires:  cargo
BuildRequires:  libxcb-devel
BuildRequires:  cairo-devel
BuildRequires:  pango-devel
BuildRequires:  lua-devel

# Disable debug package
%global debug_package %{nil}

%description
Garbar is a configurable status bar for X11 built in Rust. Features modular
widgets, Lua configuration, and cairo/pango rendering. Part of the gardesk
desktop environment suite.

%prep
%autosetup

%build
export CARGO_TARGET_DIR=target
cargo build --release --workspace

%install
install -Dm755 target/release/garbar %{buildroot}%{_bindir}/garbar
install -Dm755 target/release/garbarctl %{buildroot}%{_bindir}/garbarctl

%files
%{_bindir}/garbar
%{_bindir}/garbarctl

%changelog
* Fri Jan 17 2025 mfw <espadonne@outlook.com> - 0.1.0-1
- Initial RPM release of garbar
- Configurable status bar with modular widgets
- Lua configuration support
- Part of gardesk desktop suite
