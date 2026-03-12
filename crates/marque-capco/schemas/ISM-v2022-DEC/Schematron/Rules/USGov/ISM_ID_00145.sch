<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00145">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00145][Error] If ISM_USGOV_RESOURCE and any element in the document: 
        1. Meets ISM_CONTRIBUTES
        AND
        2. Has the attribute @ism:nonICmarkings containing [LES]
        AND
        3. No element meeting ISM_CONTRIBUTES in the document has @ism:nonICmarkings containing any of [LES-NF]
        Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [LES].
        
        Human Readable: USA documents having LES and not having LES-NF must have LES at the resource level.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If the document is an ISM_USGOV_RESOURCE, the current element is the 
      ISM_RESOURCE_ELEMENT, some element meeting ISM_CONTIBUTES specifies
      attribute @ism:nonICmarkings with a value containing the token [LES], and
      no element meeting ISM_CONTRIBUTES specifies attribute @ism:nonICmarkings
      with a value containing the token [LES-NF], then this rule ensures that
      ISM_RESOURCE_ELEMENT sepcifies attribute @ism:nonICmarkings with a value
      containing the token [LES].
    </sch:p>
    <sch:rule id="ISM-ID-00145-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and index-of($partNonICmarkings_tok, 'LES') &gt; 0 and not(index-of($partNonICmarkings_tok, 'LES-NF') &gt; 0)]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('LES'))" flag="error" role="error">
            [ISM-ID-00145][Error] If ISM_USGOV_RESOURCE and any element in the document: 
            1. Meets ISM_CONTRIBUTES
            AND
            2. Has the attribute @ism:nonICmarkings containing [LES]
            AND
            3. No element meeting ISM_CONTRIBUTES in the document has @ism:nonICmarkings containing any of [LES-NF]
            Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [LES].
            
            Human Readable: USA documents having LES and not having LES-NF must have LES at the resource level.
        </sch:assert>
    </sch:rule>
</sch:pattern>