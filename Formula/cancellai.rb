class Cancellai < Formula
  desc "Safely reclaim disk space from old Codex CLI and Claude Code session data"
  homepage "https://github.com/matteo-dritara/homebrew-cancellai"
  url "https://github.com/matteo-dritara/homebrew-cancellai/archive/refs/tags/v1.0.0.tar.gz"
  # TODO: fill in after tagging v1.0.0, e.g.:
  #   curl -L -o /tmp/cancellai.tar.gz <url above>
  #   shasum -a 256 /tmp/cancellai.tar.gz
  sha256 "REPLACE_WITH_SHA256_AFTER_TAGGING_v1_0_0"
  license "MIT"

  depends_on "python3"

  def install
    bin.install "cancellai.py" => "cancellai"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/cancellai --version")
    assert_match "cancellAI status", shell_output("#{bin}/cancellai status")
  end
end
