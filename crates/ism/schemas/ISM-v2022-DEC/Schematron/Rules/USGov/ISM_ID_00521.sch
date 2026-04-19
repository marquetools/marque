<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00521">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00521][Error] If ISM_USGOV_RESOURCE and any element: 
        1. Meets ISM_CONTRIBUTES
        AND
        2. Has the attribute @ism:disseminationControls containing [REL]
        Then the ISM_RESOURCE_ELEMENT MUST have attribute @ism:disseminationControls containing either [REL], [DISPLAYONLY] or [NF]. 
        
        Human Readable: USA documents with any portion that is REL must be one of REL, DISPLAYONLY or NF at the resource level.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_CAPCO_RESOURCE, and some element meeting ISM_CONTRIBUTES specifies
        attribute @ism:disseminationControls with a value containing [REL], 
        this rule ensures that ISM_RESOURCE_ELEMENT specifies attribute
        @ism:disseminationControls containing either the token [REL], [DISPLAYONLY] or [NF].
    </sch:p>
    <sch:rule id="ISM-ID-00521-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)
        and index-of($partDisseminationControls_tok, 'REL') &gt; 0]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL','DISPLAYONLY','NF'))" flag="error" role="error">
            [ISM-ID-00521][Error] If ISM_USGOV_RESOURCE and any element: 
            1. Meets ISM_CONTRIBUTES
            AND
            2. Has the attribute @ism:disseminationControls containing [REL]
            Then the ISM_RESOURCE_ELEMENT must have attribute @ism:disseminationControls containing either [REL], [DISPLAYONLY] or [NF]. 
            
            Human Readable: USA documents with any portion that is REL must be one of REL, DISPLAYONLY or NF at the resource level.
        </sch:assert>
    </sch:rule>
</sch:pattern>