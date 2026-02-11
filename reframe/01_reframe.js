import { op_reframe_analyze, ReframeNS } from "ext:core/ops";

export default {
  analyze(path, content, env = "server") {
    const json = ReframeNS.analyze(path, content, env);
    return JSON.parse(json);
  },
  async analyzeAsync(path, content, env = "server") {
    const json = await op_reframe_analyze(path, content, env);
    return JSON.parse(json);
  },
};
