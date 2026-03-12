<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00090">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00090][Error] If ISM_USGOV_RESOURCE and any element: 
        1. Meets ISM_CONTRIBUTES
        AND
        2. Has the attribute @ism:disseminationControls containing [REL]
        Then the ISM_RESOURCE_ELEMENT must not have attribute @ism:disseminationControls containing [EYES]. 
        
        Human Readable: USA documents with any portion that is REL must not be EYES at the resource level.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_CAPO_RESOURCE, the current element is the 
        ISM_RESOURCE_ELEMENT, and some element meeting ISM_CONTRIBUTES specifies
        attribute @ism:disseminationControls with a value containing [REL], 
        this rule ensures that ISM_RESOURCE_ELEMENT does not specify attribute
        @ism:disseminationControls or specifies the attribute with a value
        that does not contain the token [EYES].
    </sch:p>
    <sch:rule id="ISM-ID-00090-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and index-of($partDisseminationControls_tok, 'REL') &gt; 0]">
        <sch:assert test="not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('EYES')))" flag="error" role="error">
            [ISM-ID-00090][Error] If ISM_USGOV_RESOURCE and any element: 
            1. Meets ISM_CONTRIBUTES
            AND
            2. Has the attribute @ism:disseminationControls containing [REL]
            Then the ISM_RESOURCE_ELEMENT must not have attribute @ism:disseminationControls containing [EYES]. 
            
            Human Readable: USA documents with any portion that is REL must not be EYES at the resource level.
        </sch:assert>
    </sch:rule>
</sch:pattern>