# This repo is its own tap. Install with:
#   brew tap flight-william/housekeeper https://github.com/flight-william/housekeeper
#   brew install house
#
# It builds the v0.1.0 tag of this repo.
class House < Formula
  desc "Runs the tools that check the house rules and replies to agents in one format"
  homepage "https://github.com/flight-william/housekeeper"
  url "https://github.com/flight-william/housekeeper.git",
      tag: "v0.1.0"
  version "0.1.0"
  license :cannot_represent
  head "https://github.com/flight-william/housekeeper.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--locked", "--root", libexec, "--path", "."
    pkgshare.install "patterns", "scripts"

    # The binary lives in libexec; the wrapper in bin points house at the
    # installed patterns and scripts folders, so it never relies on the
    # binary's build-time default paths (gone after the build).
    (bin/"house").write_env_script libexec/"bin/house",
      HOUSE_PATTERNS_DIR: pkgshare/"patterns",
      HOUSE_SCRIPTS_DIR:  pkgshare/"scripts"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/house --version")
  end
end
