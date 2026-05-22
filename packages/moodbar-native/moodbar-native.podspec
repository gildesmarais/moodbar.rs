require "json"

package = JSON.parse(File.read(File.join(__dir__, "package.json")))

Pod::Spec.new do |s|
  s.name         = "MoodbarNative"
  s.version      = package["version"]
  s.summary      = package["description"]
  s.description  = package["description"]
  s.license      = package["license"]
  s.author       = "Moodbar"
  s.homepage     = package["homepage"]
  s.platforms    = {
    :ios => "14.0"
  }
  s.source       = {
    :git => "https://github.com/gildesmarais/moodbar.rs.git"
  }
  s.static_framework = true

  s.dependency "ExpoModulesCore"

  s.source_files = "ios/**/*.{swift,h,m,mm}"
  s.public_header_files = "ios/include/*.h"
  s.vendored_frameworks = "ios/MoodbarNativeFFI.xcframework"
  s.pod_target_xcconfig = {
    "DEFINES_MODULE" => "YES",
    "SWIFT_COMPILATION_MODE" => "wholemodule"
  }
end
