package expo.modules.moodbarnative

import android.util.Base64
import expo.modules.kotlin.exception.CodedException
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import org.json.JSONObject

class MoodbarNativeModule : Module() {
  override fun definition() = ModuleDefinition {
    Name("MoodbarNative")

    AsyncFunction("analyzeFromUri") { uri: String, optionsJson: String ->
      val payload = parseResponse(NativeBridge.nativeAnalyzeFromUri(uri, optionsJson))
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
