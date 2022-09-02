<template>
  <div>
    <h3>Check: {{ pid }} | Script: {{ id }}</h3>
    {{ content }}
  </div>
</template>

<script>
export default {
  name: "Check",
  props: ["pid", "id"],
  data() {
    return {
      content: null,
    };
  },
  created() {
    fetch(`/api/master/check_script/${this.pid}/${this.id}`, {
      method: "POST",
    })
      .then((data) => data.json())
      .then((data) => {
        if (data.success) {
          this.content = data.content;
        } else {
          console.log(data.error);
          //notify
          this.$root.notification(
            `Error running script: ${data.error}`,
            "danger",
            0
          );
        }
      })
      .catch(() => { });
  },
};
</script>

<style>
</style>