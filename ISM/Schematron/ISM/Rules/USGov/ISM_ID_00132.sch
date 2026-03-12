<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLDOWN VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00132">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00132][Error] If ISM_USGOV_RESOURCE and the
        ISM_RESOURCE_ELEMENT has the attribute @ism:disseminationControls containing [RELIDO] then every
        element meeting ISM_CONTRIBUTES_CLASSIFIED in the document must have the attribute
        @ism:disseminationControls containing [RELIDO]. 
        
        Human Readable: USA documents having RELIDO at the resource level must have every classified portion 
        having RELIDO and on any U portions that have explicit Release specified must have RELIDO. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc"> 
        If the document is an ISM_USGOV_RESOURCE, the current element is the
        ISM_RESOURCE_ELEMENT, and the ISM_RESOURCE_ELEMENT specifies the attribute
        @ism:disseminationControls with a value containing the token [RELIDO] and not an 
        unclass NF-based token (SBU-NF or LES-NF), then this rule ensures that every element 
        meeting ISM_CONTRIBUTES_CLASSIFIED specifies attribute @ism:disseminationControls 
        with a value containing the token [RELIDO]. 
    </sch:p>
    <sch:rule id="ISM-ID-00132-R1" context="*[$ISM_USGOV_RESOURCE  and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RELIDO'))]">
        <sch:assert test="every $ele in $partTags satisfies if ($ele/@ism:classification[normalize-space()='U'] and not(util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('REL','NF','DISPLAYONLY'))) and not(util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('SBU-NF', 'LES-NF')))) then true() else util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('RELIDO'))" flag="error" role="error">
            [ISM-ID-00132][Error] If ISM_USGOV_RESOURCE and the
            ISM_RESOURCE_ELEMENT has the attribute @ism:disseminationControls containing [RELIDO] then every
            element meeting ISM_CONTRIBUTES_CLASSIFIED in the document must have the attribute
            @ism:disseminationControls containing [RELIDO]. 
            
            Human Readable: USA documents having RELIDO at the resource level must have every classified portion 
            having RELIDO and on any U portions that have explicit Release specified must have RELIDO. 
        </sch:assert>
    </sch:rule>
</sch:pattern>