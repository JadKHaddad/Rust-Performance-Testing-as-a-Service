<template>
  <div>
    <h3 class="text">Preview: {{ pid }} | Script: {{ id }}</h3>
    <pre class="code">
      <code id="python-code" class="cm-s-default" ref="code-block"></code>
    </pre>
  </div>
</template>

<script>
export default {
  name: "Preview",
  props: ["pid", "id"],
  mounted() {
    fetch(`/api/preview_script/${this.pid}/${this.id}`, {
      method: "POST",
    })
      .then((data) => data.json())
      .then((data) => {
        if (data.success) {
          CodeMirror.runMode(
            "\n" + data.content,
            'python',
            this.$refs["code-block"]
          );
        } else {

        }
      })
      .catch();
  }
};
</script>

<style>

</style>