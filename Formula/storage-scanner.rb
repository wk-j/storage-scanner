# Homebrew formula. This repository is its own tap:
#
#   brew tap wk-j/storage-scanner https://github.com/wk-j/storage-scanner
#   brew install wk-j/storage-scanner/storage-scanner
#
# A new release: tag it, then point `url` at the tag's archive and set `sha256` to that
# archive's `shasum -a 256`. `--HEAD` still builds the default branch.
class StorageScanner < Formula
  desc "Native disk map with a developer tools view and manual cleanup commands"
  homepage "https://github.com/wk-j/storage-scanner"
  url "https://github.com/wk-j/storage-scanner/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "f095a1a430807d0d0669596bd5cfa6352af1d59b85e447964e6c9e601bf57ce4"
  head "https://github.com/wk-j/storage-scanner.git", branch: "main"

  depends_on "rust" => :build
  # The window renders through Metal; only the macOS build is tested.
  depends_on :macos

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    # Lists the headless scenes and exits without opening a window.
    assert_match "developer", shell_output("#{bin}/storage-scanner --list-scenes")
  end
end
