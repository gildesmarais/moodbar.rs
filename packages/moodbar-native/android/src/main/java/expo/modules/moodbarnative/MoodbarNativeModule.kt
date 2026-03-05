package expo.modules.moodbarnative

import android.net.Uri
import android.util.Base64
import expo.modules.kotlin.exception.CodedException
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import org.json.JSONObject

class MoodbarNativeModule : Module() {
  override fun definition() = ModuleDefinition {
    Name("MoodbarNative")

    AsyncFunction("analyzeFromUri") { uri: String, optionsJson: String ->
      val payload = parseResponse(analyzeUri(uri, optionsJson))
      mapOf(
        "handle" to payload.getLong("handle"),
        "frameCount" to payload.getInt("frameCount"),
        "channelCount" to payload.getInt("channelCount"),
      )
    }

    AsyncFunction("analyzeFromBase64") { base64: String, extension: String?, optionsJson: String ->
      val bytes = try {
        Base64.decode(base64, Base64.DEFAULT)
      } catch (error: IllegalArgumentException) {
        throw CodedException("ERR_INVALID_BASE64", "input bytes were not valid base64", error)
      }

      val payload = parseResponse(NativeBridge.nativeAnalyzeFromBytes(bytes, extension, optionsJson))
      mapOf(
        "handle" to payload.getLong("handle"),
        "frameCount" to payload.getInt("frameCount"),
        "channelCount" to payload.getInt("channelCount"),
      )
    }

    AsyncFunction("renderSvg") { handle: Long, optionsJson: String ->
      val payload = parseResponse(NativeBridge.nativeRenderSvg(handle, optionsJson))
      payload.getString("svg")
    }

    AsyncFunction("renderPng") { handle: Long, optionsJson: String ->
      val payload = parseResponse(NativeBridge.nativeRenderPng(handle, optionsJson))
      payload.getString("pngBase64")
    }

    AsyncFunction("disposeAnalysis") { handle: Long ->
      parseResponse(NativeBridge.nativeDisposeAnalysis(handle))
      null
    }
  }

  private fun analyzeUri(uri: String, optionsJson: String): String {
    val parsed = Uri.parse(uri)
    return when (parsed.scheme?.lowercase()) {
      "content" -> {
        val context = appContext.reactContext
          ?: throw CodedException("ERR_CONTEXT_UNAVAILABLE", "React context is unavailable for content URI decode")
        val bytes = context.contentResolver.openInputStream(parsed)?.use { it.readBytes() }
          ?: throw CodedException("ERR_URI_READ_FAILED", "could not read content URI: $uri")
        val extension = inferExtension(parsed)
        NativeBridge.nativeAnalyzeFromBytes(bytes, extension, optionsJson)
      }
      "file" -> {
        val path = parsed.path
          ?: throw CodedException("ERR_INVALID_URI", "file URI is missing a path: $uri")
        NativeBridge.nativeAnalyzeFromUri(path, optionsJson)
      }
      else -> NativeBridge.nativeAnalyzeFromUri(uri, optionsJson)
    }
  }

  private fun inferExtension(uri: Uri): String? {
    val segment = uri.lastPathSegment ?: return null
    val idx = segment.lastIndexOf('.')
    if (idx < 0 || idx == segment.length - 1) {
      return null
    }
    return segment.substring(idx + 1).lowercase()
  }

  private fun parseResponse(raw: String): JSONObject {
    val payload = JSONObject(raw)
    if (!payload.optBoolean("ok", false)) {
      throw CodedException(
        "ERR_MOODBAR_NATIVE",
        payload.optString("error", "native moodbar call failed")
      )
    }
    return payload
  }
}
