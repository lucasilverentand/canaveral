import { Text, View } from "react-native";

export default function App() {
  return (
    <View style={{ flex: 1, justifyContent: "center", alignItems: "center" }}>
      <Text>{{project_name}} / {{block_name}} expo</Text>
      <Text>API: {{api_block_name}}</Text>
    </View>
  );
}
