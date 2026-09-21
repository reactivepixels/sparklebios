# Kept up to date automatically by .github/workflows/update-formula.yml,
# which polls the sparklebios releases on a schedule and rewrites the
# version and sha256 lines below when a new one appears. Edit the
# surrounding structure here freely (dependencies, caveats, the test
# block), but a hand edit to a version or sha256 line will just be
# overwritten on the next run.
class Sparklebios < Formula
  desc "A 1995 POST screen for your terminal that is secretly a health check"
  homepage "https://github.com/reactivepixels/sparklebios"
  version "0.1.0"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_arm do
      url "https://github.com/reactivepixels/sparklebios/releases/download/v#{version}/bios-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "a9b36f29cdadb021344d5d9a069a89007a784c7fd477f69712683a819f072a8f"
    end
    on_intel do
      url "https://github.com/reactivepixels/sparklebios/releases/download/v#{version}/bios-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "0bb1e90a15fd54bd5e9ddf175987a6da368f6fd49cd9077322c16a124df9c3cb"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/reactivepixels/sparklebios/releases/download/v#{version}/bios-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "783d703f054b3256bc0a334e4c8f8991e315a3a90130504a058f81af38b4598e"
    end
    on_intel do
      url "https://github.com/reactivepixels/sparklebios/releases/download/v#{version}/bios-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "2340c66be4c6be0ec520fbc7f874b4ca9fc2625936eff98effd28c2f1c92d734"
    end
  end

  def install
    bin.install "bios"
  end

  test do
    system "#{bin}/bios", "--version"
  end
end
