# Homebrew cask template; the Release workflow fills it in and pushes it to
# the homebrew-tap repository as Casks/almanac.rb.
cask "almanac" do
  arch arm: "arm64", intel: "x86_64"

  version "@VERSION@"
  sha256 arm:   "@SHA_ARM@",
         intel: "@SHA_INTEL@"

  url "https://github.com/@REPO@/releases/download/@TAG@/Almanac-#{version}-macos-#{arch}.dmg"
  name "Almanac"
  desc "Mail, Calendar and Contacts in one app"
  homepage "https://github.com/@REPO@"

  depends_on macos: ">= :ventura"

  app "Almanac.app"

  # The app is signed ad hoc, not notarized: let macOS open it.
  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-dr", "com.apple.quarantine", "#{appdir}/Almanac.app"]
  end

  zap trash: [
    "~/Library/Application Support/Almanac",
    "~/Library/Preferences/com.craft-anything.Almanac.plist",
  ]
end
