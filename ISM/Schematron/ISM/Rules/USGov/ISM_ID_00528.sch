<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00528">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="ruleText"> [ISM-ID-00528][Error] If ISM_USGOV_RESOURCE and if
        @ism:disseminationControls contains the token [EXEMPT_FROM_ICD501_DISCOVERY] for portions
        that contribute to rollup then [EXEMPT_FROM_ICD501_DISCOVERY] must also be specified in the
        @ism:disseminationControls attribute on the ISM_RESOURCE_ELEMENT. Human Readable: If the
        token [EXEMPT_FROM_ICD501_DISCOVERY] is found in any @ism:disseminationControls in portions
        that contribute to rollup, then @disseminationControls=[EXEMPT_FROM_ICD501_DISCOVERY] must
        be rolled up to the resource level. </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="codeDesc"> If ISM_USGOV_RESOURCE, find the ISM_RESOURCE_ELEMENT and determine if
        there are any @ism:disseminationControls in portions that contribute to rollup. If there are
        any @ism:disseminationControls containing the token [EXEMPT_FROM_ICD501_DISCOVERY] in
        portions that are not @ism:excludeFromRollup="true", then ensure that the
        ISM_RESOURCE_ELEMENT has @ism:disseminationControls containing
        [EXEMPT_FROM_ICD501_DISCOVERY]. </sch:p>
    <sch:rule id="ISM-ID-00528-R1"
        context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)  
        and index-of($dcTagsFound,'EXEMPT_FROM_ICD501_DISCOVERY') &gt; 0]">
        <!-- The context is the ISM_RESOURCE_ELEMENT for a document that is a USGOV resource and has  
            @ism:disseminationControls containing EXEMPT_FROM_ICD501_DISCOVERY in portions that contribute to rollup.  -->
        <!-- If the token [EXEMPT_FROM_ICD501_DISCOVERY] is found in @ism:disseminationControls in any portion
            that contributes to rollup, then check whether the ISM_RESOURCE_ELEMENT has @ism:disseminationControls
            that contains [EXEMPT_FROM_ICD501_DISCOVERY].  If not, then error.  -->
        <sch:assert
            test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('EXEMPT_FROM_ICD501_DISCOVERY'))"
            flag="error" role="error"> [ISM-ID-00528][Error] If the token
            [EXEMPT_FROM_ICD501_DISCOVERY] is found in any @ism:disseminationControls in portions
            that contribute to rollup, then @disseminationControls=[EXEMPT_FROM_ICD501_DISCOVERY]
            must be rolled up to the resource level. </sch:assert>
    </sch:rule>
</sch:pattern>
