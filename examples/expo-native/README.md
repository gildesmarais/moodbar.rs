# Expo Native Example

Minimal Expo usage for `@moodbar/native`.

## Install

```bash
npm install @moodbar/native
```

## Example

```tsx
import { useEffect, useState } from "react";
import { Image, Text, View } from "react-native";
import { generate } from "@moodbar/native";

export default function App() {
  const [pngUri, setPngUri] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      // Replace with real local URI (file:// on iOS, file:// or content:// on Android).
      const bytes = await generate({ uri: "file:///path/to/song.mp3" }, "png", {
        width: 1200,
        height: 96,
      });

      // Persist bytes to a file in app storage and set image URI.
      // (File write omitted for brevity.)
      console.log("moodbar png bytes", bytes.length);
    })().catch((err) => {
      console.error(err);
    });
  }, []);

  return (
    <View style={{ flex: 1, justifyContent: "center", alignItems: "center" }}>
      <Text>@moodbar/native example</Text>
      {pngUri ? <Image source={{ uri: pngUri }} style={{ width: 300, height: 24 }} /> : null}
    </View>
  );
}
```
