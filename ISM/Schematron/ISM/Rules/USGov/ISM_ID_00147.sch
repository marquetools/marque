<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00147">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00147][Error] If ISM_USGOV_RESOURCE and there exist at least 2 elements in the document:
        1. Each element: Meets ISM_CONTRIBUTES
        AND
        2. One of the elements: Has the attribute @ism:nonICmarkings containing [LES-NF]
        AND
        3. One of the elements: meets ISM_CONTRIBUTES_CLASSIFIED
        Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [LES].
        
        Human Readable: Classified USA documents having LES-NF Data must have LES at the resource level.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If IC Markings System Register and Manual rules do not apply to the document then the rule does not apply
        and this rule returns true. If any element has attribute @ism:nonICmarkings specified 
        with a value containing [LES-NF] and the resourceElement has attribute @ism:classification specified 
        with a value other than [U], then this rule ensures that the resourceElement has attribute @ism:nonICmarkings
        specified with a value containing [LES].
    </sch:p>
    <sch:rule id="ISM-ID-00147-R1" context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]">
        <sch:assert test="if(not($ISM_USGOV_RESOURCE)) then true() else if(index-of($partNonICmarkings_tok, 'LES-NF') &gt; 0 and not($bannerClassification='U')) then (index-of($bannerNonICmarkings_tok, 'LES') &gt; 0) else true()" flag="error" role="error">
            [ISM-ID-00147][Error] If ISM_USGOV_RESOURCE and there exist at least 2 elements in the document:
            1. Each element: Meets ISM_CONTRIBUTES
            AND
            2. One of the elements: Has the attribute @ism:nonICmarkings containing [LES-NF]
            AND
            3. One of the elements: meets ISM_CONTRIBUTES_CLASSIFIED
            Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [LES].
            
            Human Readable: Classified USA documents having LES-NF Data must have LES at the resource level.
        </sch:assert>
    </sch:rule>
</sch:pattern>