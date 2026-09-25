class Cancellai < Formula
  desc "Safely reclaim disk space from old Codex CLI and Claude Code session data"
  homepage "https://github.com/matteo-dritara/homebrew-cancellai"
  url "https://github.com/matteo-dritara/homebrew-cancellai/archive/refs/tags/v2.0.0.tar.gz"
  sha256 "f0e4c0689125f946e7d0cd9e9742a0194d7627391149c67860d1cbd34a4aa921"
  license "MIT"

  depends_on "python3"

  on_macos do
    on_arm do
      resource "engine" do
        url "https://github.com/matteo-dritara/homebrew-cancellai/releases/download/v2.0.0/cancellai-cli-2.0.0-aarch64-apple-darwin.tar.gz"
        sha256 "d3c3d04747d2b8a9d7098ddf794a39de828d4ff05ac2b7d04fa0fbc3ef7f5f48"
      end
    end
    on_intel do
      resource "engine" do
        url "https://github.com/matteo-dritara/homebrew-cancellai/releases/download/v2.0.0/cancellai-cli-2.0.0-x86_64-apple-darwin.tar.gz"
        sha256 "5dfda6d692c8075e90fa5c9854d6fa5434892acfc362c6bbb98ea1e8686b17d3"
      end
    end
  end

  on_linux do
    on_intel do
      resource "engine" do
        url "https://github.com/matteo-dritara/homebrew-cancellai/releases/download/v2.0.0/cancellai-cli-2.0.0-x86_64-unknown-linux-gnu.tar.gz"
        sha256 "94a9ee27f43c720209eb16df890296e9f65228f4cff6e4fb7dcb40d68280776b"
      end
    end
  end

  def install
    # The Rust engine is `cancellai` (E06-S04, ADR-0039's cutover). The frozen Python reference
    # stays installed as `cancellai-legacy` through 2.1.0, as the immediate rollback.
    resource("engine").stage { bin.install "cancellai-cli" => "cancellai" }
    bin.install "cancellai.py" => "cancellai-legacy"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/cancellai version")
    assert_match version.to_s, shell_output("#{bin}/cancellai-legacy --version")
    assert_match "action(s) proposed", shell_output("#{bin}/cancellai plan")
  end
end
