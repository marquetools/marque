<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00158">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00158][Error] If ISM_USDOD_RESOURCE and:
            1. not ISM_DOD_DISTRO_EXEMPT AND
            2. attribute @ism:classification of ISM_RESOURCE_ELEMENT is not [U] AND
            3. A resource attribute @ism:noticeType does not contain one of 
               [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F].
        
        Human Readable: All classified DOD documents that do not claim
        exemption from DoD5230.24 distribution statements must use one
        of DoD distribution statements B, C, D, E, or F.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USDOD_RESOURCE and not ISM_DOD_DISTRO_EXEMPT and
        the attribute @ism:classification of ISM_RESOURCE_ELEMENT is not [U], then this rule ensures that the
        resource element specifies attribute @ism:noticeType with a value containing the token
        [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F].
    </sch:p>
    <sch:rule id="ISM-ID-00158-R1" context="*[$ISM_USDOD_RESOURCE and not($ISM_DOD_DISTRO_EXEMPT) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(@ism:classification='U')]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-B', 'DoD-Dist-C', 'DoD-Dist-D', 'DoD-Dist-E', 'DoD-Dist-F'))" flag="error" role="error">
            [ISM-ID-00158][Error] If ISM_USDOD_RESOURCE and:
            1. not ISM_DOD_DISTRO_EXEMPT AND
            2. attribute @ism:classification of ISM_RESOURCE_ELEMENT is not [U] AND
            3. A resource attribute @ism:noticeType does not contain one of 
            [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F].
            
            Human Readable: All classified DOD documents that do not claim
            exemption from DoD5230.24 distribution statements must use one
            of DoD distribution statements B, C, D, E, or F.
        </sch:assert>
    </sch:rule>
</sch:pattern>