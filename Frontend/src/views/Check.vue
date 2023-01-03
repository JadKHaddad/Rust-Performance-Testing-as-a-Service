<template>
  <div>
    <h3 class="text">Check: {{ pid }} | Script: {{ id }}</h3>
    <div v-if="loading" uk-spinner></div>
    <div class="check-content text">{{ content }}</div>
  </div>
</template>

<script>
export default {
  name: "Check",
  props: ["pid", "id"],
  data() {
    return {
      content: null,
      loading: false
    };
  },
  created() {
    this.loading = true;
    fetch(`/api/check_script/${this.pid}/${this.id}`, {
      method: "POST",
    })
      .then((data) => data.json())
      .then((data) => {
        this.loading = false;
        if (data.success) {
          this.content = data.content;
          console.log(data);
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
      .catch(() => { this.loading = false; });
  },
};
</script>

<style>
</style>