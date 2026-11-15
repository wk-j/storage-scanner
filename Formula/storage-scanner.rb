# Homebrew formula. This repository is its own tap:
#
#   brew tap wk-j/storage-scanner https://github.com/wk-j/storage-scanner
#   brew install --HEAD wk-j/storage-scanner/storage-scanner
#
# There is no tagged release yet, so the formula builds the default branch (--HEAD).
class StorageScanner < Formula
  desc "Native disk map with a developer tools view and manual cleanup commands"
  homepage "https://github.com/wk-j/storage-scanner"
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
