<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       --><sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00088">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00088][Error] If ISM_USGOV_RESOURCE and @ism:releasableTo is specified on the resource
        element then all classified portions must specify @ism:releasableTo and all Unclass portions must be REL or contain
        no caveats. 
        
        Human Readable: USA documents having any classified portion that is not Releasable or having
        unclassified portions with disseminationControls that are not [REL] cannot be REL at the resource level.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If IC Markings System Register and Manual rules apply to the document, this rule verifies
        that all portions either have the attribute @ism:classification specified with a value of [U] and uncaveated or REL
        or classified portions of the document have the attribute @ism:releasableTo. Attribute @ism:releasableTo is only valid on
        an element if attribute @ism:disseminationControls is specified with a value containing [REL] or [EYES], as [REL]
        supersedes [EYES] in the banner. If any elements do not meet either of the two requirements stated above, then
        the assertion fails since attribute @ism:releasableTo appears on the banner but is not present on all classified
        portions.
    </sch:p>
    <sch:rule id="ISM-ID-00088-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:releasableTo]">
        <sch:assert test="every $portion in $partTags satisfies ( ($portion/@ism:classification='U' and not($portion/@ism:disseminationControls) ) or $portion/@ism:releasableTo[normalize-space()])" flag="error" role="error">
            [ISM-ID-00088][Error] If ISM_USGOV_RESOURCE and @ism:releasableTo is specified on the resource
            element then all classified portions must specify @ism:releasableTo and all Unclass portions must be REL or contain
            no caveats. 
            
            Human Readable: USA documents having any classified portion that is not Releasable or having
            unclassified portions with disseminationControls that are not [REL] cannot be REL at the resource level.
        </sch:assert>
    </sch:rule>
</sch:pattern>