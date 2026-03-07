import ExpoModulesCore
import Foundation

public class MoodbarNativeModule: Module {
  public func definition() -> ModuleDefinition {
    Name("MoodbarNative")

    AsyncFunction("analyzeFromUri") { (uri: String, optionsJson: String?) throws -> [String: Any] in
      let resolvedPath = try self.resolvePath(from: uri)
      var summary = MoodbarNativeAnalysisSummary(handle: 0, frame_count: 0, channel_count: 0)
      let status = resolvedPath.withCString { pathPtr in
        withOptionalCString(optionsJson) { optionsPtr in
          moodbar_native_analysis_from_path(pathPtr, optionsPtr, &summary)
        }
      }
      try throwIfNeeded(status)
      return [
        "handle": summary.handle,
        "frameCount": Int(summary.frame_count),
        "channelCount": Int(summary.channel_count),
      ]
    }

    AsyncFunction("analyzeFromBase64") { (base64: String, extension: String?, optionsJson: String?) throws -> [String: Any] in
      guard let data = Data(base64Encoded: base64) else {
        throw NSError(domain: "MoodbarNative", code: 1, userInfo: [NSLocalizedDescriptionKey: "input bytes were not valid base64"])
      }

      var summary = MoodbarNativeAnalysisSummary(handle: 0, frame_count: 0, channel_count: 0)
      let status = data.withUnsafeBytes { rawBuffer in
        let bytes = rawBuffer.bindMemory(to: UInt8.self).baseAddress
        return withOptionalCString(extension) { extensionPtr in
          withOptionalCString(optionsJson) { optionsPtr in
            moodbar_native_analysis_from_bytes(bytes, data.count, extensionPtr, optionsPtr, &summary)
          }
        }
      }

      try throwIfNeeded(status)
      return [
        "handle": summary.handle,
        "frameCount": Int(summary.frame_count),
        "channelCount": Int(summary.channel_count),
      ]
    }

    AsyncFunction("renderSvg") { (handle: UInt64, optionsJson: String?) throws -> String in
      var out = MoodbarNativeBuffer(ptr: nil, len: 0, cap: 0)
      defer { moodbar_native_buffer_free(&out) }
      let status = withOptionalCString(optionsJson) { optionsPtr in
        moodbar_native_render_svg(handle, optionsPtr, &out)
      }
      try throwIfNeeded(status)

      let data = consumeBuffer(out)
      guard let svg = String(data: data, encoding: .utf8) else {
        throw NSError(domain: "MoodbarNative", code: 3, userInfo: [NSLocalizedDescriptionKey: "native SVG payload was not UTF-8"])
      }
      return svg
    }

    AsyncFunction("renderPng") { (handle: UInt64, optionsJson: String?) throws -> String in
      var out = MoodbarNativeBuffer(ptr: nil, len: 0, cap: 0)
      defer { moodbar_native_buffer_free(&out) }
      let status = withOptionalCString(optionsJson) { optionsPtr in
        moodbar_native_render_png(handle, optionsPtr, &out)
      }
      try throwIfNeeded(status)

      let data = consumeBuffer(out)
      return data.base64EncodedString()
    }

    AsyncFunction("disposeAnalysis") { (handle: UInt64) throws -> Void in
      let status = moodbar_native_analysis_dispose(handle)
      try throwIfNeeded(status)
    }
  }

  private func withOptionalCString<T>(_ value: String?, _ body: (UnsafePointer<CChar>?) -> T) -> T {
    guard let value else {
      return body(nil)
    }
    return value.withCString { cString in
      body(cString)
    }
  }

  private func throwIfNeeded(_ status: MoodbarNativeStatus) throws {
    if status == 0 {
      return
    }

    var out = MoodbarNativeBuffer(ptr: nil, len: 0, cap: 0)
    _ = moodbar_native_last_error(&out)
    defer { moodbar_native_buffer_free(&out) }

    let data = consumeBuffer(out)
    let message = String(data: data, encoding: .utf8) ?? "native moodbar call failed"
    throw NSError(
      domain: "MoodbarNative",
      code: Int(status),
      userInfo: [NSLocalizedDescriptionKey: message]
    )
  }

  private func consumeBuffer(_ buffer: MoodbarNativeBuffer) -> Data {
    guard let ptr = buffer.ptr else {
      return Data()
    }
    return Data(bytes: ptr, count: buffer.len)
  }

  private func resolvePath(from uri: String) throws -> String {
    guard let url = URL(string: uri), url.scheme != nil else {
      return uri
    }

    if url.isFileURL {
      return url.path
    }

    throw NSError(
      domain: "MoodbarNative",
      code: 4,
      userInfo: [NSLocalizedDescriptionKey: "unsupported URI scheme for iOS: \(uri)"]
    )
  }
}
