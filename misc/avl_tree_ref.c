#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#define min(a, b) ((a) < (b) ? (a) : (b))
#define max(a, b) ((a) > (b) ? (a) : (b))

#define N 1000
#define i16 int16_t

#define T INT16_MAX
#define MIN INT16_MIN

struct node {
    i16 low;
    i16 high;
    i16 max;
    i16 left;
    i16 right;
    i16 parent;
    i16 height;
};

i16 root = T;
i16 len = 0;
struct node nodes[N];

void init_node(i16 i, i16 low, i16 high)
{
    nodes[i].low = low;
    nodes[i].high = high;
    nodes[i].max = high;
    nodes[i].left = T;
    nodes[i].right = T;
    nodes[i].parent = T;
    nodes[i].height = 1;
}

i16 height(i16 x)
{
    if (x == T)
        return 0;

    return nodes[x].height;
}

i16 diff(i16 x)
{
    return height(nodes[x].right) - height(nodes[x].left);
}

void update_height(i16 x)
{
    i16 lh = height(nodes[x].left);
    i16 rh = height(nodes[x].right);

    nodes[x].height = 1 + max(lh, rh);
}

void update_max(i16 x)
{
    i16 lm = nodes[x].left == T ? MIN : nodes[nodes[x].left].max;
    i16 rm = nodes[x].right == T ? MIN : nodes[nodes[x].right].max;

    nodes[x].max = max(nodes[x].high, max(lm, rm));
}

i16 right_rotate(i16 x)
{
    i16 y = nodes[x].left;

    nodes[x].left = nodes[y].right;

    if (nodes[y].right != T)
        nodes[nodes[y].right].parent = x;

    nodes[y].parent = nodes[x].parent;

    if (nodes[x].parent == T)
        root = y;
    else if (x == nodes[nodes[x].parent].left)
        nodes[nodes[x].parent].left = y;
    else
        nodes[nodes[x].parent].right = y;

    nodes[y].right = x;
    nodes[x].parent = y;

    update_height(x);
    update_height(y);

    update_max(x);
    update_max(y);

    return y;
}

i16 left_rotate(i16 x)
{
    i16 y = nodes[x].right;

    nodes[x].right = nodes[y].left;

    if (nodes[y].left != T)
        nodes[nodes[y].left].parent = x;

    nodes[y].parent = nodes[x].parent;

    if (nodes[x].parent == T)
        root = y;
    else if (x == nodes[nodes[x].parent].left)
        nodes[nodes[x].parent].left = y;
    else
        nodes[nodes[x].parent].right = y;

    nodes[y].left = x;
    nodes[x].parent = y;

    update_height(x);
    update_height(y);

    update_max(x);
    update_max(y);

    return y;
}

i16 balance(i16 x)
{
    i16 d = diff(x);

    if (d > 1) {
        if (diff(nodes[x].right) < 0)
            nodes[x].right = right_rotate(nodes[x].right);

        return left_rotate(x);
    }

    if (d < -1) {
        if (diff(nodes[x].left) > 0)
            nodes[x].left = left_rotate(nodes[x].left);

        return right_rotate(x);
    }

    update_height(x);
    update_max(x);

    return x;
}

void insert(i16 low, i16 high)
{
    i16 n = len++;
    init_node(n, low, high);

    if (root == T) {
        root = n;
        return;
    }

    i16 x = root;
    i16 p = T;

    while (x != T) {
        p = x;

        if (low < nodes[x].low)
            x = nodes[x].left;
        else
            x = nodes[x].right;
    }

    if (low < nodes[p].low)
        nodes[p].left = n;
    else
        nodes[p].right = n;

    nodes[n].parent = p;

    x = n;

    while (nodes[x].parent != T) {
        x = nodes[x].parent;
        x = balance(x);
    }

    root = x;
    return;
}

bool overlap(i16 x0, i16 x1, i16 y0, i16 y1)
{
    return x0 <= y1 && y0 <= x1;
}

i16 search(i16 low, i16 high)
{
    i16 x = root;

    while (x != T && !overlap(low, high, nodes[x].low, nodes[x].high)) {
        i16 left = nodes[x].left;

        if (left != T && nodes[left].max >= low)
            x = left;
        else
            x = nodes[x].right;
    }

    return x;
}

void find_all_overlapping(i16 x, i16 low, i16 high, i16* results, i16* rlen)
{
    if (x == T)
        return;

    if (overlap(low, high, nodes[x].low, nodes[x].high))
        results[(*rlen)++] = x;

    if (nodes[x].left != T && nodes[nodes[x].left].max >= low)
        find_all_overlapping(nodes[x].left, low, high, results, rlen);

    if (nodes[x].right != T && nodes[nodes[x].right].max >= low)
        find_all_overlapping(nodes[x].right, low, high, results, rlen);
}

void printer(i16 x, int level)
{
    if (x == T)
        return;

    for (int i = 1; i <= level * 4; ++i)
        printf(" ");

    printf("[%d,%d] %d\n", nodes[x].low, nodes[x].high, nodes[x].max);

    printer(nodes[x].right, level + 1);
    printer(nodes[x].left, level + 1);
}

void print()
{
    printer(root, 0);
}

void gather_values(i16 x, i16* values, i16* len)
{
    values[(*len)++] = nodes[x].low;

    if (nodes[x].left != T)
        gather_values(nodes[x].left, values, len);

    if (nodes[x].right != T)
        gather_values(nodes[x].right, values, len);
}

void check_inequality(i16 x)
{
    i16 l = nodes[x].left;
    i16 r = nodes[x].right;

    if (l != T) {
        i16 *values = malloc(N * sizeof(i16));
        i16 len = 0;

        gather_values(l, values, &len);

        for (int i = 0; i < len; ++i)
            assert(values[i] <= nodes[x].low);

        free(values);

        check_inequality(l);
    }

    if (r != T) {
        i16 *values = malloc(N * sizeof(i16));
        i16 len = 0;

        gather_values(r, values, &len);

        for (int i = 0; i < len; ++i)
            assert(values[i] >= nodes[x].low);

        free(values);

        check_inequality(r);
    }
}

i16 calc_height(i16 x)
{
    if (x == T)
        return 0;

    i16 l = calc_height(nodes[x].left);
    i16 r = calc_height(nodes[x].right);

    return 1 + max(l, r);
}

void check_height(i16 x)
{
    assert(calc_height(x) == nodes[x].height);

    if (nodes[x].left != T)
        check_height(nodes[x].left);

    if (nodes[x].right != T)
        check_height(nodes[x].right);
}

i16 calc_max(i16 x)
{
    if (x == T)
        return MIN;

    i16 l = calc_max(nodes[x].left);
    i16 r = calc_max(nodes[x].right);
    i16 t = nodes[x].high;

    return max(t, max(l, r));
}

void check_max(i16 x)
{
    assert(calc_max(x) == nodes[x].max);

    if (nodes[x].left != T)
        check_max(nodes[x].left);

    if (nodes[x].right != T)
        check_max(nodes[x].right);
}

void check_invariants()
{
    check_inequality(root);
    check_height(root);
    check_max(root);
}

void find_all_overlapping_naive(i16 low, i16 high, i16* actual, i16* alen)
{
    for (i16 i = 0; i < len; ++i)
        if (overlap(low, high, nodes[i].low, nodes[i].high))
            actual[(*alen)++] = i;
}

void check_overlaps(i16 *results, i16 rlen, i16 *actual, i16 alen)
{
    assert(rlen == alen);

    for (i16 i = 0; i < rlen; ++i) {
        bool found = false;

        for (i16 j = 0; j < alen; ++j)
            if (actual[j] == results[i]) {
                found = true;
                break;
            }

        assert(found);
    }
}

void test_overlaps()
{
    i16 x = root;
    while (nodes[x].left != T)
        x = nodes[x].left;

    i16 start = nodes[x].low;
    i16 end = nodes[root].max;

    for (i16 i = start; i <= end; ++i)
        for (i16 j = i; j <= end; ++j) {
            i16 *results = malloc(N * sizeof(i16));
            i16 *actual = malloc(N * sizeof(i16));
            i16 rlen = 0;
            i16 alen = 0;

            find_all_overlapping(root, i, j, results, &rlen);
            find_all_overlapping_naive(i, j, actual, &alen);

            check_overlaps(results, rlen, actual, alen);

            free(results);
            free(actual);
        }
}

void test()
{
    int num_tests = 0;

    while (1) {
        ++num_tests;

        printf("test=%d\n", num_tests);

        srand(num_tests);

        root = T;
        len = 0;

        int num_intervals = 300 + rand() % 300;

        for (int i = 0; i < num_intervals; ++i) {
            int low = 1 + rand() % 200;
            int high = low + rand() % 200;

            insert(low, high);
        }

        check_invariants();

        test_overlaps();
    }
}

int main()
{
    test();
}
