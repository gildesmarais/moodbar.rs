package expo.modules.moodbarnative

object NativeBridge {
  init {
    System.loadLibrary("moodbar_native_ffi")
  }

  external fun nativeAnalyzeFromUri(uri: String, optionsJson: String): String
  external fun nativeAnalyzeFromBytes(bytes: ByteArray, extension: String?, optionsJson: String): String
  external fun nativeRenderSvg(handle: Long, optionsJson: String): String
  external fun nativeRenderPng(handle: Long, optionsJson: String): String
  external fun nativeDisposeAnalysis(handle: Long): String
}
